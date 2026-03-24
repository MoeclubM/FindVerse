use crate::models::IndexedDocument;
use std::collections::HashSet;

pub struct SpamDetector;

impl SpamDetector {
    pub fn is_spam(document: &IndexedDocument) -> bool {
        let mut spam_score = 0.0;

        if Self::keyword_stuffing(&document.body) {
            spam_score += 0.3;
        }

        if Self::excessive_links(&document.body) {
            spam_score += 0.2;
        }

        if document.body.len() < 100 {
            spam_score += 0.2;
        }

        if Self::repetitive_content(&document.body) {
            spam_score += 0.3;
        }

        spam_score > 0.5
    }

    fn keyword_stuffing(text: &str) -> bool {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() < 10 {
            return false;
        }
        let unique_words: HashSet<&str> = words.iter().copied().collect();
        let ratio = unique_words.len() as f32 / words.len() as f32;
        ratio < 0.3
    }

    fn excessive_links(text: &str) -> bool {
        let link_count = text.matches("http").count();
        let word_count = text.split_whitespace().count();
        if word_count < 50 {
            return false;
        }
        link_count as f32 / word_count as f32 > 0.1
    }

    fn repetitive_content(text: &str) -> bool {
        let sentences: Vec<&str> = text.split('.').collect();
        if sentences.len() < 5 {
            return false;
        }
        let unique: HashSet<&str> = sentences.iter().copied().collect();
        (unique.len() as f32) / (sentences.len() as f32) < 0.5
    }
}
