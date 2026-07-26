//! Fuzzy matching and ranking shared across the gator app family.

/// Rank of a match, ordered lexicographically (smaller is better):
/// `(penalty, span, gaps, start, text_len)` where `penalty` is 0 for a
/// contiguous case-insensitive substring and 1 for a scattered subsequence.
pub type MatchScore = (usize, usize, usize, usize, usize);

/// Whether every non-whitespace character of `query` appears in `text` in
/// order (case-insensitive subsequence match). An empty query always matches.
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut query_chars = query.chars().filter(|c| !c.is_whitespace());
    let mut current = query_chars.next();
    if current.is_none() {
        return true;
    }
    for ch in text.chars() {
        if let Some(expected) = current {
            if expected.eq_ignore_ascii_case(&ch) {
                current = query_chars.next();
                if current.is_none() {
                    return true;
                }
            }
        }
    }
    false
}

/// Score how well `query` matches `text`, or `None` if it does not match.
///
/// A contiguous case-insensitive substring scores best (penalty 0); otherwise a
/// scattered subsequence scores by span and gap tightness (penalty 1). Compare
/// the returned tuples directly to rank candidates.
pub fn match_score(query: &str, text: &str) -> Option<MatchScore> {
    let qchars: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
    if qchars.is_empty() {
        return Some((0, 0, 0, 0, text.chars().count()));
    }

    if let Some(start) = find_case_insensitive(text, query) {
        let span = qchars.len().saturating_sub(1);
        return Some((0, span, 0, start, text.chars().count()));
    }

    let mut positions: Vec<usize> = Vec::with_capacity(qchars.len());
    let mut qi = 0usize;
    for (ti, t) in text.chars().enumerate() {
        if qi >= qchars.len() {
            break;
        }
        if qchars[qi].eq_ignore_ascii_case(&t) {
            positions.push(ti);
            qi += 1;
        }
    }

    if qi < qchars.len() {
        return None;
    }

    let start = *positions.first().unwrap_or(&0);
    let end = *positions.last().unwrap_or(&start);
    let span = end.saturating_sub(start);
    let mut gaps = 0usize;
    for window in positions.windows(2) {
        if let [prev, next] = window {
            gaps = gaps.saturating_add(next.saturating_sub(prev + 1));
        }
    }
    let text_len = text.chars().count();
    Some((1, span, gaps, start, text_len))
}

/// Char index of the first case-insensitive occurrence of `needle` in `text`,
/// or `None`. An empty needle matches at 0.
pub fn find_case_insensitive(text: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let text_lower = text.to_lowercase();
    let needle_lower = needle.to_lowercase();
    let byte_index = text_lower.find(&needle_lower)?;
    Some(char_index_from_byte(text, byte_index))
}

/// Convert a byte offset into `text` to a char offset.
pub fn char_index_from_byte(text: &str, byte_index: usize) -> usize {
    text.char_indices()
        .take_while(|(idx, _)| *idx < byte_index)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_is_case_insensitive_subsequence() {
        assert!(fuzzy_match("ac", "abc"));
        assert!(!fuzzy_match("ca", "abc"));
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn substring_outranks_scattered_subsequence() {
        let contiguous = match_score("bc", "abcd").expect("match");
        let scattered = match_score("bd", "abcd").expect("match");
        assert_eq!(contiguous.0, 0);
        assert_eq!(scattered.0, 1);
        assert!(contiguous < scattered);
        assert!(match_score("zz", "abcd").is_none());
    }
}
