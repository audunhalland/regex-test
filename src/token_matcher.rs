use std::collections::BTreeSet;

pub mod automaton_matcher;
pub mod hash_matcher;
pub mod regex_matcher;
pub mod regex_util;
pub mod test_util;

///
/// Abstraction over tantivy searcher with the functionality this module needs:
///
pub trait GetDocFreq {
    fn get_doc_freq(&self, term: &crate::Term) -> u64;
}

///
/// Doc Freq Reciprocal - a reciprocal of the doc freq!
///
/// A doc freq of 1 yields 1/2
/// A doc freq of 2 yields 1/3
/// etc.
///
/// This is used for scoring individual snippets fragments, etc.
///
#[derive(Clone, Debug)]
pub struct DocFreqReciprocal(pub f32);

impl DocFreqReciprocal {
    fn from_doc_freq(doc_freq: u64) -> Option<DocFreqReciprocal> {
        if doc_freq == 0 {
            None
        } else {
            Some(DocFreqReciprocal(1.0 / (doc_freq as f32 + 1.0)))
        }
    }
}

#[cfg(test)]
impl PartialEq<DocFreqReciprocal> for DocFreqReciprocal {
    fn eq(&self, other: &DocFreqReciprocal) -> bool {
        !(self.0 < other.0) && !(self.0 > other.0)
    }
}

///
/// All things a token matcher can match for:
///
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum MatchPredicate {
    Term(String),
    Pattern(crate::PatternAST),
}

///
/// The set of predicates used for one matcher instance:
///
pub type MatchPredicateSet = BTreeSet<MatchPredicate>;

///
/// Trait for the external API of the matcher itself, that snippet generators and highlighters use.
///
pub trait LookupDocFreqReciprocal {
    ///
    /// Lookup up DocFreqReciprocal for a token.
    ///
    /// self is mut because it is common to cache results internally as it is progressing.
    ///
    fn lookup_doc_freq_reciprocal(
        &mut self,
        token_text: &str,
        get_doc_freq: &impl GetDocFreq,
    ) -> Option<DocFreqReciprocal>;
}

pub mod test {
    use super::*;

    use crate::PerfTimer;
    use std::collections::HashMap;

    struct FooBarBazTermDb;

    impl GetDocFreq for FooBarBazTermDb {
        fn get_doc_freq(&self, term: &crate::Term) -> u64 {
            match term.text() {
                "foo" => 1,
                "bar" => 2,
                "baz" => 3,
                _ => 0,
            }
        }
    }

    struct AnyTermDb;

    impl GetDocFreq for AnyTermDb {
        fn get_doc_freq(&self, _: &crate::Term) -> u64 {
            1
        }
    }

    fn assert_matcher_matches(
        matcher: &mut impl LookupDocFreqReciprocal,
        matcher_name: &str,
        patterns: &[&[&str]],
        token_text: &str,
        expected: bool,
    ) {
        match matcher.lookup_doc_freq_reciprocal(token_text, &AnyTermDb) {
            Some(_) => {
                if !expected {
                    panic!(
                        "{}: Pattern {:?} should not match {}, but did",
                        matcher_name, patterns, token_text
                    )
                }
            }
            None => {
                if expected {
                    panic!(
                        "{}: Pattern {:?} should match {}, but didn't",
                        matcher_name, patterns, token_text
                    )
                }
            }
        };
    }

    fn assert_matches(patterns: &[&[&str]], token_text: &str, expected: bool) {
        const MATCH_N: usize = 400;
        let mut perf_timer = PerfTimer::new();

        {
            let mut matcher = regex_matcher::test::test_regex_matcher(patterns);
            perf_timer.add_milestone("re::comp");
            for _ in 0..MATCH_N {
                assert_matcher_matches(&mut matcher, "regex", patterns, token_text, expected);
            }
            perf_timer.add_milestone("re::match");
        }

        {
            let mut matcher = automaton_matcher::test::test_automaton_matcher(patterns);
            perf_timer.add_milestone("au::comp");
            for _ in 0..MATCH_N {
                assert_matcher_matches(&mut matcher, "automaton", patterns, token_text, expected);
            }
            perf_timer.add_milestone("au::match");
        }

        {
            let mut hashmap: HashMap<String, String> = HashMap::new();
            for pattern in patterns {
                hashmap.insert(pattern.join(""), "fooooobar".to_string());
            }
            perf_timer.add_milestone("hm::comp");
            for _ in 0..MATCH_N {
                let _ = hashmap.get(token_text);
            }
            perf_timer.add_milestone("hm::match");
        }

        println!("{:?} matching \"{}\"", patterns, token_text);

        for dur in perf_timer.durations() {
            println!("    {:?}", dur);
        }

        println!();
    }

    pub fn test_actual_matcher_implementations() {
        assert_matches(&[&["a"]], "a", true);
        assert_matches(&[&["a"]], "b", false);
        assert_matches(&[&["a", "*"]], "a", true);
        assert_matches(&[&["a", "*"]], "ab", true);
        assert_matches(&[&["a", "*"]], "ba", false);
        assert_matches(&[&["*", "a"]], "ba", true);
        assert_matches(&[&["*", "a"]], "ab", false);
        assert_matches(&[&["*", "a"]], "aba", true);
        assert_matches(&[&["*", "a", "*"]], "bar", true);
        assert_matches(&[&["*", "a", "*"]], "foo", false);
        assert_matches(&[&["f", "*", "r"]], "foobar", true);
        assert_matches(&[&["f", "*", "r"]], "barfoo", false);
        assert_matches(&[&["f", "*", "r"]], "uforg", false);

        assert_matches(
            &[
                &["nå"],
                &["må"],
                &["vi"],
                &["teste"],
                &["mange"],
                &["forskjellige"],
                &["mønstere"],
                &["fu", "*", "k!"],
                &["wildca", "*"],
            ],
            "furusjenk!",
            true,
        );

        assert_matches(
            &[
                &["have"],
                &["to"],
                &["test"],
                &["many"],
                &["different"],
                &["patterns"],
                &["cl", "*", "k!"],
                &["wildca", "*"],
            ],
            "clugulububiffjoglufagofibonggrik!",
            true,
        );

        assert_matches(
            &[
                &["have"],
                &["to"],
                &["test"],
                &["many"],
                &["different"],
                &["patterns"],
                &["cl", "*", "k!"],
                &["wildca", "*"],
            ],
            "clugrik!",
            true,
        );

        assert_matches(
            &[
                &["mang", "*"],
                &["mønste", "*"],
                &["me", "*"],
                &["wildc", "*"],
                &["e", "*"],
                &["no", "*"],
                &["v", "*"],
                &["ogs", "*"],
                &["m", "*"],
                &["tes", "*"],
                &["veldi", "*"],
                &["br", "*"],
                &["go", "*"],
                &["hel", "*"],
            ],
            "mønstergjenkjenning",
            true,
        );

        assert_matches(
            &[
                &["mang", "*"],
                &["mønste", "*"],
                &["me", "*"],
                &["wildc", "*"],
                &["e", "*"],
                &["no", "*"],
                &["v", "*"],
                &["ogs", "*"],
                &["m", "*"],
                &["tes", "*"],
                &["veldi", "*"],
                &["br", "*"],
                &["go", "*"],
                &["hel", "*"],
                &["mul", "*", "gens"],
                &["b", "*", "r"],
                &["m", "*", "n"],
                &["og", "*", "å"],
                &["ne", "*", "ne"],
                &["wil", "*", "rd"],
                &["int", "*", "rnt"],
            ],
            "muligens",
            true,
        );

        assert_matches(
            &[
                &["mang", "*"],
                &["mønste", "*"],
                &["me", "*"],
                &["wildc", "*"],
                &["e", "*"],
                &["no", "*"],
                &["v", "*"],
                &["ogs", "*"],
                &["m", "*"],
                &["tes", "*"],
                &["veldi", "*"],
                &["br", "*"],
                &["go", "*"],
                &["hel", "*"],
                &["mul", "*", "gens"],
                &["b", "*", "r"],
                &["m", "*", "n"],
                &["og", "*", "å"],
                &["ne", "*", "ne"],
                &["wil", "*", "rd"],
                &["int", "*", "rnt"],
                &["*", "gså"],
                &["*", "ldcards"],
                &["*", "eksten"],
            ],
            "muligens",
            true,
        );

        // Failing this test so that we'll see the println!s:
        assert!(false);
    }
}
