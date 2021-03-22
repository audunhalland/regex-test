use std::collections::HashMap;

use super::*;

///
/// Very simple (and fast!) matcher that only works on terms, not patterns.
///
/// This matcher should be used if there are no wildcard queries to process.
///
pub struct HashMatcher {
    term_doc_freq_reciprocals_map: HashMap<String, DocFreqReciprocal>,
}

impl HashMatcher {
    pub fn new(term_doc_freq_reciprocals_map: &HashMap<String, DocFreqReciprocal>) -> Self {
        Self {
            term_doc_freq_reciprocals_map: term_doc_freq_reciprocals_map.clone(),
        }
    }
}

impl LookupDocFreqReciprocal for HashMatcher {
    fn lookup_doc_freq_reciprocal(
        &mut self,
        token_text: &str,
        _get_doc_freq: &impl GetDocFreq,
    ) -> Option<DocFreqReciprocal> {
        self.term_doc_freq_reciprocals_map
            .get(token_text)
            .map(|dfr| dfr.clone())
    }
}
