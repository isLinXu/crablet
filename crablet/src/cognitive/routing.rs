use crate::error::Result;
use crate::types::Message;

pub mod complexity {
    use super::*;

    #[derive(Clone, Debug, Default)]
    pub struct TaskCharacteristics {
        pub word_count: usize,
        pub sentence_count: usize,
        pub technical_terms: usize,
        pub question_count: usize,
        pub instruction_count: usize,
        pub creativity_score: f32,
        pub detected_domains: Vec<String>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct ComplexityAnalyzer;

    impl ComplexityAnalyzer {
        pub fn new() -> Self {
            Self
        }

        pub fn extract_characteristics(&self, messages: &[Message]) -> Result<TaskCharacteristics> {
            let text = messages
                .iter()
                .filter_map(|m| m.text())
                .collect::<Vec<_>>()
                .join(" ");

            let word_count = text.split_whitespace().count();
            let sentence_count = text.matches(['.', '!', '?', '。', '！', '？']).count().max(1);
            let question_count = text.matches(['?', '？']).count();

            Ok(TaskCharacteristics {
                word_count,
                sentence_count,
                question_count,
                ..Default::default()
            })
        }

        pub fn calculate_complexity_score(&self, c: &TaskCharacteristics) -> f32 {
            let base = (c.word_count as f32 / 30.0) + (c.question_count as f32 * 0.5);
            base.clamp(0.0, 10.0)
        }
    }
}
