use crate::PatternASTNode;
use super::*;

///
/// Patterns grouped into 5 groups:
/// 1. terms (no wildcards)
/// 2. terms_wc: ends with a wildcard, but does not start with a wildcard
/// 3. terms_internal_wc: does not start nor end with a wildcard, but has internal wildcards
/// 4. wc_terms: starts with a wildcard, but does not end with a wildcard
/// 5. wc_terms_wc: starts and ends with a wildcard
///
/// the groups have their wildcard at start/end stripped away.
///
/// This grouping is done in order to optimize automaton compile times, where we can group
/// together various wildcards.
/// e.g.:
/// ".*(foo|bar)" instead of "(.*foo)|(.*bar)"
///
#[derive(Default)]
pub struct GroupedPatterns<'a> {
    pub terms: Vec<&'a str>,
    pub terms_wc: Vec<&'a [PatternASTNode]>,
    pub terms_internal_wc: Vec<&'a [PatternASTNode]>,
    pub wc_terms: Vec<&'a [PatternASTNode]>,
    pub wc_terms_wc: Vec<&'a [PatternASTNode]>,
}

impl<'a> GroupedPatterns<'a> {
    pub fn group(predicate_set: &'a BTreeSet<MatchPredicate>) -> Self {
        let mut groups = GroupedPatterns::default();

        for match_predicate in predicate_set {
            match match_predicate {
                MatchPredicate::Term(term_text) => {
                    groups.terms.push(term_text);
                }
                MatchPredicate::Pattern(ast) => {
                    let nodes = &ast.0;
                    match nodes.first() {
                        Some(PatternASTNode::Literal(first_text)) => {
                            if nodes.len() == 1 {
                                groups.terms.push(first_text);
                            } else {
                                match nodes.last() {
                                    Some(PatternASTNode::Literal(_)) => {
                                        groups.terms_internal_wc.push(nodes);
                                    }
                                    Some(PatternASTNode::Wildcard) => {
                                        groups.terms_wc.push(&nodes[..nodes.len() - 1]);
                                    }
                                    None => {}
                                }
                            }
                        }
                        Some(PatternASTNode::Wildcard) => {
                            if nodes.len() > 1 {
                                match nodes.last() {
                                    Some(PatternASTNode::Literal(_)) => {
                                        groups.wc_terms.push(&nodes[1..]);
                                    }
                                    Some(PatternASTNode::Wildcard) => {
                                        groups.wc_terms_wc.push(&nodes[1..nodes.len() - 1]);
                                    }
                                    None => {}
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
        }

        groups
    }
}
