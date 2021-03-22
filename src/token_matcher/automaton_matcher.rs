use regex_automata::dense::DenseDFA;
use std::collections::HashMap;
use std::sync::Arc;

use regex_automata::DFA;

use crate::PatternASTNode;

use super::*;

///
/// A regex automata that can be re-used by matchers.
/// This allows for ridiculously fast searches.
/// At the expense of very slow compile time.
///
pub struct Automaton {
    dense_dfa: DenseDFA<Vec<usize>, usize>,
}

pub struct AutomatonMatcher {
    automaton: Arc<Automaton>,
    doc_freq_cache: HashMap<String, Option<DocFreqReciprocal>>,
    term_buf: crate::Term,
}

impl AutomatonMatcher {
    pub fn new(
        automaton: Arc<Automaton>,
        predicate_set: &MatchPredicateSet,
        term_doc_freq_reciprocals: &HashMap<String, DocFreqReciprocal>,
    ) -> Self {
        let mut doc_freq_cache: HashMap<String, Option<DocFreqReciprocal>> = HashMap::new();

        for match_predicate in predicate_set {
            if let MatchPredicate::Term(term_text) = match_predicate {
                doc_freq_cache.insert(
                    term_text.to_string(),
                    term_doc_freq_reciprocals
                        .get(term_text)
                        .map(|dfr| dfr.clone()),
                );
            }
        }

        Self {
            automaton,
            doc_freq_cache,
            term_buf: crate::Term::default(),
        }
    }

    fn text_term(&mut self, token_text: &str) -> &crate::Term {
        self.term_buf.set_text(token_text);
        &self.term_buf
    }
}

impl LookupDocFreqReciprocal for AutomatonMatcher {
    fn lookup_doc_freq_reciprocal(
        &mut self,
        token_text: &str,
        get_doc_freq: &impl GetDocFreq,
    ) -> Option<DocFreqReciprocal> {
        let match_length = self.automaton.dense_dfa.find(token_text.as_bytes())?;
        if match_length < token_text.len() {
            return None;
        }

        // We got a match, now need to find doc_freq:
        if let Some(doc_freq_reciprocal) = self.doc_freq_cache.get(token_text) {
            return doc_freq_reciprocal.clone();
        }

        let term = self.text_term(token_text);
        let doc_freq_reciprocal = DocFreqReciprocal::from_doc_freq(get_doc_freq.get_doc_freq(term));

        self.doc_freq_cache
            .insert(token_text.to_string(), doc_freq_reciprocal.clone());

        doc_freq_reciprocal
    }
}

const WILDCARD_EXPR: &str = r#"[\x{0000}-\x{024f}]*"#;

pub fn compile_automaton(predicate_set: &MatchPredicateSet) -> Result<Arc<Automaton>, String> {
    let regex_pattern = generate_regex_pattern(predicate_set, WILDCARD_EXPR);

    println!("au pattern: {}", regex_pattern);

    // CPU usage alert:
    let dense_dfa = regex_automata::dense::Builder::new()
        .anchored(true)
        .build(&regex_pattern)
        .map_err(|error| format!("compile_automaton failed. {:?}", error))?;

    Ok(Arc::new(Automaton { dense_dfa }))
}

fn generate_regex_pattern(predicate_set: &BTreeSet<MatchPredicate>, wildcard_expr: &str) -> String {
    let groups = super::regex_util::GroupedPatterns::group(predicate_set);

    let regex_exprs: Vec<Option<String>> = vec![
        if groups.terms.len() > 0 {
            Some(
                groups
                    .terms
                    .into_iter()
                    .map(regex_syntax::escape)
                    .collect::<Vec<_>>()
                    .join("|"),
            )
        } else {
            None
        },
        if groups.terms_wc.len() > 0 {
            Some(format!(
                "(({}){})",
                pattern_asts_to_regex_string(&groups.terms_wc, wildcard_expr),
                wildcard_expr,
            ))
        } else {
            None
        },
        if groups.terms_internal_wc.len() > 0 {
            Some(pattern_asts_to_regex_string(
                &groups.terms_internal_wc,
                wildcard_expr,
            ))
        } else {
            None
        },
        if groups.wc_terms.len() > 0 {
            Some(format!(
                "({}({}))",
                wildcard_expr,
                pattern_asts_to_regex_string(&groups.wc_terms, wildcard_expr)
            ))
        } else {
            None
        },
        if groups.wc_terms_wc.len() > 0 {
            Some(format!(
                "({}({}){})",
                wildcard_expr,
                pattern_asts_to_regex_string(&groups.wc_terms_wc, wildcard_expr),
                wildcard_expr,
            ))
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

fn pattern_asts_to_regex_string(pattern_asts: &[&[PatternASTNode]], wildcard_expr: &str) -> String {
    pattern_asts
        .into_iter()
        .map(|ast_nodes| match ast_nodes.len() {
            0 => None,
            1 => match ast_nodes.first() {
                Some(PatternASTNode::Literal(text)) => Some(regex_syntax::escape(text)),
                _ => None,
            },
            _ => Some(format!(
                "({})",
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
        })
        .filter_map(|opt| opt)
        .collect::<Vec<_>>()
        .join("|")
}

// #[cfg(test)]
pub mod test {
    use super::test_util;
    use super::*;

    use crate::PerfTimer;

    pub fn test_automaton_matcher(patterns: &[&[&str]]) -> AutomatonMatcher {
        let predicate_set = test_util::create_predicate_set(patterns);
        let term_doc_freq_reciprocals =
            test_util::term_doc_freq_reciprocals_from_predicate_set(&predicate_set);
        let automaton = compile_automaton(&predicate_set).unwrap();

        AutomatonMatcher::new(automaton, &predicate_set, &term_doc_freq_reciprocals)
    }

    fn test_generate_regex_pattern(patterns: &[&[&str]]) -> String {
        generate_regex_pattern(&test_util::create_predicate_set(patterns), ".*")
    }

    #[test]
    fn generate_regex_pattern_works_with_empty_input() {
        assert_eq!(test_generate_regex_pattern(&[]), "".to_string());
    }

    #[test]
    fn generate_regex_pattern_works_with_literal_terms_only() {
        assert_eq!(
            test_generate_regex_pattern(&[&["foo"], &["bar"],]),
            "bar|foo".to_string()
        );
    }

    #[test]
    fn generate_regex_pattern_works_with_every_pattern_end_wildcarded() {
        assert_eq!(
            test_generate_regex_pattern(&[&["foo", "*"], &["bar", "*"], &["baz", "*"]]),
            "((bar|baz|foo).*)".to_string()
        );
    }

    #[test]
    fn generate_regex_pattern_works_with_types_from_each_group() {
        assert_eq!(
            test_generate_regex_pattern(&[
                &["a"],
                &["*", "b"],
                &["c", "*"],
                &["*", "d", "*"],
                &["e", "*", "f"],
                &["g"],
                &["*", "h"],
                &["i", "*"],
                &["*", "j", "*"],
                &["k", "*", "l"]
            ]),
            "a|g|((c|i).*)|(e.*f)|(k.*l)|(.*(b|h))|(.*(d|j).*)".to_string()
        );
    }

    #[test]
    fn generate_regex_pattern_escapes_literals() {
        assert_eq!(
            test_generate_regex_pattern(&[&["o.s.v."], &["*", "lol(?)"],]),
            r#"o\.s\.v\.|(.*(lol\(\?\)))"#.to_string()
        );
    }

    #[test]
    #[ignore = "enable this test to help analyzing automaton compile times"]
    fn test_various_dfa() {
        fn test(pattern: &'static str, expect: &[&str]) {
            let mut perf_timer = PerfTimer::new();

            let dfa = regex_automata::dense::Builder::new()
                .anchored(true)
                .build(pattern)
                .unwrap();
            perf_timer.add_milestone(pattern);

            for input in expect {
                match dfa.find(input.as_bytes()) {
                    Some(length) => {
                        if length != input.len() {
                            panic!(
                                "Pattern {} should match _all_ of {}, but matched {}",
                                pattern, input, length
                            );
                        }
                    }
                    None => {
                        panic!("Pattern {} should match {}", pattern, input);
                    }
                }
            }

            println!(
                "{:?}           mem: {}",
                perf_timer.durations(),
                dfa.memory_usage()
            );
        }

        println!("term:");
        test("foo", &["foo"]);
        test("foo|bar", &["bar"]);
        test("foo|bar|baz", &["baz"]);
        println!("term*:");
        test("foo.*", &["foobar"]);
        test("(foo|bar).*", &["foobar", "barfoo"]);
        test("(foo|bar|qux).*", &["foobar", "barfoo"]);
        test("(foo|bar|qux|baflebiflegæfle).*", &["foobar", "barfoo"]);
        test(
            r#"(foo|bar|qux|baflebiflegæfle)[\x{0000}-\x{024f}]*"#,
            &["foobar", "barfoo"],
        );
        println!("*term:");
        test(".*foo", &["lolzofoo"]);
        test(".*(foo|bar)", &["lolzobar"]);
        test(".*(foo|bar|qux)", &["gufluqux"]);
        test(".*(foo|bar|qux|tifletæfletøfle)", &["lobzqux"]);
        test(
            r#"[\x{0000}-\x{024f}]*(foo|bar|qux|tifletæfletøfle)"#,
            &["lobzqux"],
        );
        println!("*term*:");
        test(".*foo.*", &["bazfoobarqux"]);
        test(".*(foo|bar).*", &["bazfoobarqux"]);
        test(".*(foo|bar|baz).*", &["bazfoobarqux"]);
        test(".*(foo|bar|baz|qux).*", &["bazfoobarqux"]);
        test(".*(foo|bar|baz|qux|nasleroflerable).*", &["bazfoobarqux"]);
        test(
            r#"[\x{0000}-\x{024f}]*(foo|bar|baz|qux|nasleroflerable)[\x{0000}-\x{024f}]*"#,
            &["bazfoobarqux"],
        );
        // println!("term|term*");
        println!("term|*term*:");
        //  + wc/unions/wc:
        test("(foo|bar)|(.*baz.*)", &["bar", "labbazzab"]);
        test(
            "(foo|bar)|(.*baz.*)|(.*qux.*)",
            &["foo", "zabazios", "luquxus"],
        );
        test("(foo|bar)|(.*(baz|qux).*)", &["foo", "zabazios", "luquxus"]);
        test(
            "(foo|bar)|(.*baz.*)|(.*qux.*)|(.*loff.*)",
            &["bar", "labbazzab", "goloffofoblo"],
        );
        test(
            "(foo|bar)|(.*(baz|qux|loff).*)",
            &["bar", "labbazzab", "goloffofoblo"],
        );
        test(
            "(foo|bar)|(.*baz.*)|(.*qux.*)|(.*køll.*)|(.*livl.*)|(.*lobz.*)|(.*wægg.*)|(.*knofl.*)",
            &["foo", "zabazios", "luquxus"],
        );
        test(
            "(foo|bar)|(.*(baz|qux|køll|livl|lobz|wægg|knofl).*)",
            &["foo", "zabazios", "luquxus"],
        );
        test(
            "(foo|bar)|(.*(baz|qux|køll|livl|lobz|wægg|knofl).*)",
            &["foo", "zabazios", "luquxus"],
        );
        println!("term|*te*rm*:");
        test("(foo|bar)|(.*(baz|qux).*)|(.*foo.*bar.*)", &[]);
        test("(foo|bar)|(.*(baz|qux|(foo.*bar)).*)", &[]);
        test(
            "(foo|bar|lol|lobbings|sibbos|gælk)|(.*(baz|qux).*)|(.*foo.*bar.*)",
            &["gælk", "læffoogoobarlox"],
        );

        assert!(false);
    }
}
