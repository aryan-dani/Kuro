//! Deduplication of hallucinated phrase and sentence repetitions from STT.

/// Deduplicate consecutive identical sentences, clauses, or n-gram loops.
pub fn dedup_repeats(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 1. Sentence / clause-level deduplication
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut current_punct = String::new();

    for c in trimmed.chars() {
        if c == '.' || c == '?' || c == '!' {
            current_punct.push(c);
        } else if !current_punct.is_empty() {
            segments.push((current.trim().to_string(), current_punct.clone()));
            current.clear();
            current_punct.clear();
            current.push(c);
        } else {
            current.push(c);
        }
    }
    if !current.trim().is_empty() || !current_punct.is_empty() {
        segments.push((current.trim().to_string(), current_punct));
    }

    let mut deduped_segments: Vec<(String, String)> = Vec::new();
    for (content, punct) in segments {
        if content.is_empty() {
            continue;
        }
        let is_dup = if let Some((prev_content, _)) = deduped_segments.last() {
            prev_content.eq_ignore_ascii_case(&content)
        } else {
            false
        };
        if !is_dup {
            // Check if this segment is a trailing fragment/prefix of the previous segment
            let is_trailing_fragment = if let Some((prev_content, _)) = deduped_segments.last() {
                let prev_lower = prev_content.to_ascii_lowercase();
                let cur_lower = content.to_ascii_lowercase();
                prev_lower.starts_with(&cur_lower) && cur_lower.len() < prev_lower.len()
            } else {
                false
            };
            if !is_trailing_fragment {
                deduped_segments.push((content, punct));
            }
        }
    }

    let mut reconstructed = String::new();
    for (content, punct) in deduped_segments {
        if !reconstructed.is_empty() {
            reconstructed.push(' ');
        }
        reconstructed.push_str(&content);
        reconstructed.push_str(&punct);
    }

    // 2. Word-level n-gram loop deduplication (for repetitions without punctuation)
    dedup_word_ngrams(&reconstructed)
}

fn normalize_word(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase()
}

fn dedup_word_ngrams(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() < 4 {
        return text.to_string();
    }

    let normalized: Vec<String> = tokens.iter().map(|&w| normalize_word(w)).collect();
    let n_tokens = tokens.len();

    // Check for repeating blocks of size n from n_tokens / 2 down to 2
    for block_len in (2..=(n_tokens / 2)).rev() {
        let mut i = 0;
        let mut found_repeat = false;
        while i + 2 * block_len <= n_tokens {
            let b1 = &normalized[i..i + block_len];
            let b2 = &normalized[i + block_len..i + 2 * block_len];
            if b1 == b2 {
                found_repeat = true;
                break;
            }
            i += 1;
        }

        if found_repeat {
            let mut result_tokens = Vec::new();
            let mut i = 0;
            while i < n_tokens {
                if i + 2 * block_len <= n_tokens {
                    let b1 = &normalized[i..i + block_len];
                    let b2 = &normalized[i + block_len..i + 2 * block_len];
                    if b1 == b2 {
                        // Push first block, skip all consecutive identical blocks
                        for t in &tokens[i..i + block_len] {
                            result_tokens.push(*t);
                        }
                        i += block_len;
                        while i + block_len <= n_tokens && &normalized[i..i + block_len] == b1 {
                            i += block_len;
                        }
                        // Also skip any trailing partial prefix of b1
                        let remaining = n_tokens - i;
                        if remaining > 0 && remaining < block_len && normalized[i..n_tokens] == b1[..remaining] {
                            i = n_tokens;
                        }
                        continue;
                    }
                }
                result_tokens.push(tokens[i]);
                i += 1;
            }
            return result_tokens.join(" ");
        }
    }

    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_repeats_sentence_loop() {
        let input = "Click on join now. Click on join now.";
        assert_eq!(dedup_repeats(input), "Click on join now.");
    }

    #[test]
    fn test_dedup_repeats_multi_loop_with_fragment() {
        let input = "Why are you so weak at all. Why are you so weak at all. Why are you so weak at all. Why are you";
        assert_eq!(dedup_repeats(input), "Why are you so weak at all.");
    }

    #[test]
    fn test_dedup_repeats_dad_meet() {
        let input = "Send a Google Meet link to Dad. Send a Google Meet link to Dad. Send a Google Meet link to Dad.";
        assert_eq!(dedup_repeats(input), "Send a Google Meet link to Dad.");
    }

    #[test]
    fn test_dedup_repeats_no_cut_off() {
        let input = "Do not cut me off. Do not cut me off.";
        assert_eq!(dedup_repeats(input), "Do not cut me off.");
    }

    #[test]
    fn test_dedup_repeats_unpunctuated_words() {
        let input = "this is mad this is mad this is mad";
        assert_eq!(dedup_repeats(input), "this is mad");
    }

    #[test]
    fn test_dedup_preserves_normal_speech() {
        let input = "Hello there. How are you doing today?";
        assert_eq!(dedup_repeats(input), "Hello there. How are you doing today?");
    }
}
