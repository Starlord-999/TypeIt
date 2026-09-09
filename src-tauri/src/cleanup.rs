/// Lighter cleanup for a mid-utterance streaming chunk: normalizes
/// whitespace and drops common Whisper hallucinations on short/quiet audio
/// (e.g. "[BLANK_AUDIO]", "[Music]"), but skips forced capitalization and
/// ending punctuation since the chunk isn't the end of the sentence.
pub fn cleanup_partial(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_non_speech_hallucination(trimmed) {
        return String::new();
    }
    trimmed.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn is_non_speech_hallucination(text: &str) -> bool {
    let bracketed = text.starts_with('[') && text.ends_with(']');
    let parenthesized = text.starts_with('(') && text.ends_with(')');
    bracketed || parenthesized
}

pub fn cleanup_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Normalize multiple spaces to single space
    let normalized: String = trimmed
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    // Capitalize first letter of each sentence
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in normalized.chars() {
        if capitalize_next && ch.is_alphabetic() {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
            if ch == '.' || ch == '!' || ch == '?' {
                capitalize_next = true;
            }
        }
    }

    // Ensure ending punctuation
    if let Some(last) = result.chars().last() {
        if !matches!(last, '.' | '!' | '?') {
            result.push('.');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_whitespace() {
        assert_eq!(cleanup_text("  hello world  "), "Hello world.");
    }

    #[test]
    fn test_normalize_spaces() {
        assert_eq!(cleanup_text("hello    world"), "Hello world.");
    }

    #[test]
    fn test_capitalize_first_letter() {
        assert_eq!(cleanup_text("hello world"), "Hello world.");
    }

    #[test]
    fn test_capitalize_after_period() {
        assert_eq!(cleanup_text("hello. world"), "Hello. World.");
    }

    #[test]
    fn test_capitalize_after_question_mark() {
        assert_eq!(cleanup_text("hello? world"), "Hello? World.");
    }

    #[test]
    fn test_capitalize_after_exclamation() {
        assert_eq!(cleanup_text("hello! world"), "Hello! World.");
    }

    #[test]
    fn test_ensure_ending_punctuation() {
        assert_eq!(cleanup_text("hello world"), "Hello world.");
    }

    #[test]
    fn test_preserve_existing_ending_punctuation() {
        assert_eq!(cleanup_text("hello world."), "Hello world.");
        assert_eq!(cleanup_text("hello world!"), "Hello world!");
        assert_eq!(cleanup_text("hello world?"), "Hello world?");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(cleanup_text(""), "");
        assert_eq!(cleanup_text("   "), "");
    }

    #[test]
    fn test_already_clean() {
        assert_eq!(cleanup_text("Hello world."), "Hello world.");
    }

    #[test]
    fn test_partial_no_forced_capitalization_or_punctuation() {
        assert_eq!(cleanup_partial("hello world"), "hello world");
    }

    #[test]
    fn test_partial_normalizes_spaces() {
        assert_eq!(cleanup_partial("hello   world  "), "hello world");
    }

    #[test]
    fn test_partial_drops_bracketed_hallucination() {
        assert_eq!(cleanup_partial("[BLANK_AUDIO]"), "");
        assert_eq!(cleanup_partial("[Music]"), "");
    }

    #[test]
    fn test_partial_drops_parenthesized_hallucination() {
        assert_eq!(cleanup_partial("(silence)"), "");
    }

    #[test]
    fn test_partial_empty() {
        assert_eq!(cleanup_partial(""), "");
        assert_eq!(cleanup_partial("   "), "");
    }
}
