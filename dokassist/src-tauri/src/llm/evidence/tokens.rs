//! Token accounting for evidence assembly.
//!
//! Assembly needs a token count before a model is necessarily loaded, so the
//! budget is expressed against a [`TokenCounter`]. The heuristic counter is
//! used in tests and when no engine is available; the engine implementation
//! uses the loaded model's own tokenizer, which is what the context budget is
//! actually measured in.

use crate::llm::engine::LlmEngine;

/// Anything that can measure a prompt fragment in tokens.
pub trait TokenCounter {
    fn count(&self, text: &str) -> usize;
    /// Label recorded in the manifest so a reader knows how exact the counts are.
    fn label(&self) -> &'static str;
}

/// Model-independent upper estimate: German clinical text runs about three
/// characters per token, and never fewer tokens than whitespace-separated
/// words.
pub fn estimate_tokens(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    chars.div_ceil(3).max(words).max(1)
}

/// Tokenizer-free counter. Overestimates slightly, which keeps assembly inside
/// the real context window.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicCounter;

impl TokenCounter for HeuristicCounter {
    fn count(&self, text: &str) -> usize {
        estimate_tokens(text)
    }

    fn label(&self) -> &'static str {
        "heuristic"
    }
}

impl TokenCounter for LlmEngine {
    fn count(&self, text: &str) -> usize {
        self.count_tokens(text)
    }

    fn label(&self) -> &'static str {
        "model-tokenizer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_costs_nothing() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("   \n"), 0);
    }

    #[test]
    fn estimate_grows_with_length_and_never_underruns_word_count() {
        let short = estimate_tokens("Patient stabil");
        let long = estimate_tokens(&"Patient stabil ".repeat(50));
        assert!(short >= 2);
        assert!(long > short * 10);

        let many_short_words = "a ".repeat(30);
        assert!(estimate_tokens(&many_short_words) >= 30);
    }

    #[test]
    fn heuristic_counter_matches_estimate() {
        let counter = HeuristicCounter;
        assert_eq!(
            counter.count("Sertralin 50 mg"),
            estimate_tokens("Sertralin 50 mg")
        );
        assert_eq!(counter.label(), "heuristic");
    }
}
