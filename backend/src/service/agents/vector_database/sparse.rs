use std::collections::HashMap;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is", "it",
    "its", "of", "on", "that", "the", "to", "was", "were", "will", "with", "the", "this", "but",
    "they", "have", "had", "what", "when", "where", "who", "which", "or", "not", "no", "so", "if",
    "out", "up", "do", "my", "me", "we", "i", "you",
];

/// Tokenize text: lowercase, split on non-alphanumeric, remove stopwords and single chars
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .filter(|t| !STOPWORDS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Generate sparse vector from text using term frequency weights.
/// Returns (indices, values) where indices are FNV-hashed token IDs.
pub fn text_to_sparse_vector(text: &str) -> (Vec<u32>, Vec<f32>) {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return (vec![], vec![]);
    }

    let mut tf: HashMap<String, u32> = HashMap::new();
    for token in &tokens {
        *tf.entry(token.clone()).or_insert(0) += 1;
    }

    let total = tokens.len() as f32;
    let mut pairs: Vec<(u32, f32)> = tf
        .iter()
        .map(|(token, count)| (fnv_hash(token), (*count as f32) / total))
        .collect();

    // Sort by index for consistent ordering
    pairs.sort_by_key(|(idx, _)| *idx);
    pairs.into_iter().unzip()
}

/// FNV-1a hash to map tokens to u32 indices
fn fnv_hash(s: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for byte in s.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}
