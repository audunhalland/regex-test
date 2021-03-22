use std::collections::HashMap;

use crate::PatternASTNode;

use super::*;

pub struct RegexMatcher {
    regex: regex::Regex,
    capture_locations_buf: regex::CaptureLocations,

    term_count: usize,

    // A vector of term doc freqs, indexed by the Regex' intial term groups
    term_doc_freq_reciprocals: Vec<Option<DocFreqReciprocal>>,
    pattern_doc_freq_cache: HashMap<String, Option<DocFreqReciprocal>>,

    term_buf: crate::Term,
}

impl RegexMatcher {
    pub fn new(
        regex: regex::Regex,
        predicate_set: &MatchPredicateSet,
        term_doc_freq_reciprocals_map: &HashMap<String, DocFreqReciprocal>,
    ) -> Self {
        let mut term_doc_freq_reciprocals: Vec<Option<DocFreqReciprocal>> = vec![];
        let mut term_count = 0;

        for match_predicate in predicate_set {
            if let MatchPredicate::Term(term_text) = match_predicate {
                term_doc_freq_reciprocals.push(
                    term_doc_freq_reciprocals_map
                        .get(term_text)
                        .map(|dfr| dfr.clone()),
                );
                term_count += 1;
            }
        }

        let capture_locations_buf = regex.capture_locations();

        Self {
            regex,
            capture_locations_buf,
            term_count,
            term_doc_freq_reciprocals,
            pattern_doc_freq_cache: HashMap::new(),
            term_buf: crate::Term::default(),
        }
    }

    fn text_term(&mut self, token_text: &str) -> &crate::Term {
        self.term_buf.set_text(token_text);
        &self.term_buf
    }
}

impl LookupDocFreqReciprocal for RegexMatcher {
    fn lookup_doc_freq_reciprocal(
        &mut self,
        token_text: &str,
        get_doc_freq: &impl GetDocFreq,
    ) -> Option<DocFreqReciprocal> {
        let _ = self
            .regex
            .captures_read(&mut self.capture_locations_buf, token_text)?;

        // Loop through terms and see if we find the doc_freq_reciprocal
        // BUG: is this really faster than using a HashMap?
        for term_index in 0..self.term_count {
            if let Some(_) = self.capture_locations_buf.get(term_index + 1) {
                return self
                    .term_doc_freq_reciprocals
                    .get(term_index)
                    .and_then(|dfr| dfr.clone());
            }
        }

        let opt_pattern_doc_freq = self.pattern_doc_freq_cache.get(token_text);

        if let Some(pattern_doc_freq) = opt_pattern_doc_freq {
            return pattern_doc_freq.clone();
        }

        let term = self.text_term(token_text);
        let doc_freq_reciprocal = DocFreqReciprocal::from_doc_freq(get_doc_freq.get_doc_freq(term));

        self.pattern_doc_freq_cache
            .insert(token_text.to_string(), doc_freq_reciprocal.clone());

        doc_freq_reciprocal
    }
}

enum CompileStrategy {
    VeryFlat,
    Flat,
    Grouped,
}

pub fn compile_regex(predicate_set: &MatchPredicateSet) -> Result<regex::Regex, String> {
    let regex_pattern = generate_regex_pattern(predicate_set, r#"[\x{0000}-\x{024f}]*"#);

    println!("re pattern: {}", regex_pattern);

    regex::Regex::new(&regex_pattern).map_err(|error| format!("compile_regex failed. {:?}", error))
}

fn generate_regex_pattern(predicate_set: &BTreeSet<MatchPredicate>, wildcard_expr: &str) -> String {
    let groups = super::regex_util::GroupedPatterns::group(predicate_set);

    let regex_exprs: Vec<Option<String>> = vec![
        if groups.terms.len() > 0 {
            Some(
                groups
                    .terms
                    .into_iter()
                    .map(|term| format!("^({})$", regex_syntax::escape(term)))
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        } else {
            None
        },
        if groups.terms_internal_wc.len() > 0 {
            Some(
                groups
                    .terms_internal_wc
                    .into_iter()
                    .map(|pattern| pattern_to_regex_expr(pattern, wildcard_expr))
                    .filter_map(|opt| opt.map(|expr| format!("^{}$", expr)))
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        } else {
            None
        },
        if groups.terms_wc.len() > 0 {
            Some(
                groups
                    .terms_wc
                    .into_iter()
                    .map(|pattern| pattern_to_regex_expr(pattern, wildcard_expr))
                    .filter_map(|opt| opt.map(|expr| format!("^{}", expr)))
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        } else {
            None
        },
        if groups.wc_terms.len() > 0 {
            Some(
                groups
                    .wc_terms
                    .into_iter()
                    .map(|pattern| pattern_to_regex_expr(pattern, wildcard_expr))
                    .filter_map(|opt| opt.map(|expr| format!("{}$", expr)))
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        } else {
            None
        },
        if groups.wc_terms_wc.len() > 0 {
            Some(
                groups
                    .wc_terms_wc
                    .into_iter()
                    .map(|pattern| pattern_to_regex_expr(pattern, wildcard_expr))
                    .filter_map(|opt| opt)
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        } else {
            None
        },
    ];

    regex_exprs
        .into_iter()
        .filter_map(|opt| opt)
        .collect::<Vec<_>>()
        .join("|")
}

fn patterns_to_regex_expr(pattern_asts: &[&[PatternASTNode]], wildcard_expr: &str) -> String {
    pattern_asts
        .into_iter()
        .map(|ast_nodes| pattern_to_regex_expr(ast_nodes, wildcard_expr))
        .filter_map(|opt| opt)
        .collect::<Vec<_>>()
        .join("|")
}

fn pattern_to_regex_expr(ast_nodes: &[PatternASTNode], wildcard_expr: &str) -> Option<String> {
    match ast_nodes.len() {
        0 => None,
        1 => match ast_nodes.first() {
            Some(PatternASTNode::Literal(text)) => Some(regex_syntax::escape(text)),
            // No "*"!
            _ => None,
        },
        _ => Some(format!(
            // "(?:{})",
            "{}",
            ast_nodes
                .into_iter()
                .map(|node| {
                    match node {
                        PatternASTNode::Literal(text) => regex_syntax::escape(text),
                        PatternASTNode::Wildcard => wildcard_expr.to_string(),
                    }
                })
                .collect::<Vec<_>>()
                .join("")
        )),
    }
}

#[cfg(test)]
pub mod test {
    use super::test_util;
    use super::*;

    pub fn test_regex_matcher(patterns: &[&[&str]]) -> RegexMatcher {
        let predicate_set = test_util::create_predicate_set(patterns);
        let term_doc_freq_reciprocals =
            test_util::term_doc_freq_reciprocals_from_predicate_set(&predicate_set);

        RegexMatcher::new(
            compile_regex(&predicate_set).unwrap(),
            &predicate_set,
            &term_doc_freq_reciprocals,
        )
    }

    fn test_generate_regex_pattern(patterns: &[&[&str]]) -> String {
        generate_regex_pattern(&test_util::create_predicate_set(patterns), ".*")
    }
}
