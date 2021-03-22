#![allow(dead_code)]

mod token_matcher;

///
/// Data type representing the pattern elements
/// that we support
///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PatternASTNode {
    Literal(String),
    Wildcard,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PatternAST(pub Vec<PatternASTNode>);

/// A search "Term" - based on https://docs.rs/tantivy/0.14.0/tantivy/struct.Term.html
#[derive(Default)]
pub struct Term(pub Vec<u8>);

impl Term {
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap()
    }

    pub fn set_text(&mut self, text: &str) {
        self.0 = text.as_bytes().iter().cloned().collect();
    }
}

///
/// Utility for timing things
///
#[derive(Clone, Debug)]
pub struct PerfTimer {
    pub start_instant: std::time::Instant,
    pub milestones: Vec<(&'static str, std::time::Duration)>,
}

impl PerfTimer {
    pub fn new() -> Self {
        Self {
            start_instant: std::time::Instant::now(),
            milestones: vec![],
        }
    }

    pub fn add_milestone(&mut self, name: &'static str) {
        self.milestones.push((name, self.start_instant.elapsed()));
    }

    pub fn durations(&self) -> Vec<(&'static str, std::time::Duration)> {
        let mut prev_duration = std::time::Duration::from_secs(0);
        let mut durations = vec![];

        for (name, duration) in self.milestones.iter() {
            let current_duration = duration.checked_sub(prev_duration).unwrap();
            durations.push((*name, current_duration));
            prev_duration = *duration;
        }

        durations
    }
}

fn main() {
    token_matcher::test::test_actual_matcher_implementations();
}
