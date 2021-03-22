use std::collections::HashMap;

use crate::*;
use super::*;

///
/// Generate a match predicate set from test patterns, e.g.:
/// &[&["bar"], &["*", "foo"]]
/// a pattern of length 1 becomes a term predicate, length > 1 becomes a pattern predicate.
///
pub fn create_predicate_set(patterns: &[&[&str]]) -> MatchPredicateSet {
    patterns
        .iter()
        .map(|pattern| {
            if pattern.len() == 1 {
                MatchPredicate::Term(pattern[0].to_string())
            } else {
                MatchPredicate::Pattern(PatternAST(
                    pattern
                        .iter()
                        .map(|str| match *str {
                            "*" => PatternASTNode::Wildcard,
                            _ => PatternASTNode::Literal(str.to_string()),
                        })
                        .collect(),
                ))
            }
        })
        .collect()
}

pub fn term_doc_freq_reciprocals_from_predicate_set(
    predicate_set: &MatchPredicateSet,
) -> HashMap<String, DocFreqReciprocal> {
    let mut term_doc_freq_reciprocals: HashMap<String, DocFreqReciprocal> = HashMap::new();

    for match_predicate in predicate_set.iter() {
        if let MatchPredicate::Term(term_text) = match_predicate {
            term_doc_freq_reciprocals.insert(
                term_text.to_owned(),
                DocFreqReciprocal::from_doc_freq(1).unwrap(),
            );
        }
    }

    term_doc_freq_reciprocals
}
