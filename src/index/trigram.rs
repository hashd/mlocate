pub fn generate_trigrams(path: &str) -> Vec<String> {
    if path.is_empty() {
        return vec!["___".to_string()];
    }

    let chars: Vec<char> = path.chars().collect();
    if chars.len() < 3 {
        let padded: String = std::iter::once('_')
            .chain(chars.iter().copied())
            .chain(std::iter::once('_'))
            .collect();
        let pchars: Vec<char> = padded.chars().collect();
        if pchars.len() >= 3 {
            let tri: String = pchars[..3].iter().collect();
            return vec![tri];
        }
        return vec![padded];
    }

    let mut trigrams = Vec::with_capacity(chars.len() - 2);
    for i in 0..=(chars.len() - 3) {
        let tri: String = chars[i..i + 3].iter().collect();
        trigrams.push(tri);
    }

    trigrams.sort();
    trigrams.dedup();
    trigrams
}

pub fn generate_trigrams_lowercase(path: &str) -> Vec<String> {
    let folded = casefold(path);
    generate_trigrams(&folded)
}

pub fn generate_bigrams(path: &str) -> Vec<String> {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() < 2 {
        return Vec::new();
    }

    let mut bigrams = Vec::with_capacity(chars.len() - 1);
    for i in 0..=(chars.len() - 2) {
        let bi: String = chars[i..i + 2].iter().collect();
        bigrams.push(bi);
    }

    bigrams.sort();
    bigrams.dedup();
    bigrams
}

pub fn generate_bigrams_lowercase(path: &str) -> Vec<String> {
    let folded = casefold(path);
    generate_bigrams(&folded)
}

// Uses caseless v0.2 which implements Unicode 12.0 case folding.
// Current Unicode is version 16+. File paths are overwhelmingly ASCII,
// so this divergence is unlikely to affect real-world use.
pub fn casefold(s: &str) -> String {
    let normalized = unicode_normalization::UnicodeNormalization::nfc(s);
    caseless::default_case_fold_str(&normalized.collect::<String>())
}

pub fn trigram_to_bytes(trigram: &str) -> [u8; 3] {
    let bytes = trigram.as_bytes();
    let mut arr = [0u8; 3];
    let len = bytes.len().min(3);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

pub fn bigram_to_bytes(bigram: &str) -> [u8; 2] {
    let bytes = bigram.as_bytes();
    let mut arr = [0u8; 2];
    let len = bytes.len().min(2);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trigrams_basic() {
        let tris = generate_trigrams("foo");
        assert_eq!(tris, vec!["foo"]);
    }

    #[test]
    fn test_generate_trigrams_longer() {
        let tris = generate_trigrams("foobar");
        assert_eq!(tris, vec!["bar", "foo", "oba", "oob"]);
    }

    #[test]
    fn test_generate_trigrams_short() {
        let tris = generate_trigrams("/a");
        assert_eq!(tris, vec!["_/a"]);
    }

    #[test]
    fn test_generate_trigrams_two_char() {
        let tris = generate_trigrams("/ab");
        assert_eq!(tris, vec!["/ab"]);
    }

    #[test]
    fn test_generate_trigrams_dedup() {
        let tris = generate_trigrams("/aaa");
        assert_eq!(tris, vec!["/aa", "aaa"]);
    }

    #[test]
    fn test_generate_trigrams_lowercase() {
        let tris = generate_trigrams_lowercase("README.md");
        assert_eq!(tris, vec![".md", "adm", "dme", "e.m", "ead", "me.", "rea"]);
    }

    #[test]
    fn test_casefold_unicode() {
        let tris = generate_trigrams_lowercase("M\u{00dc}LLER");
        let expected = generate_trigrams("m\u{00fc}ller");
        assert_eq!(tris, expected);
    }

    #[test]
    fn test_generate_bigrams() {
        let bis = generate_bigrams("foo.rs");
        assert_eq!(bis, vec![".r", "fo", "o.", "oo", "rs"]);
    }

    #[test]
    fn test_generate_bigrams_short() {
        let bis = generate_bigrams("f");
        assert!(bis.is_empty());
    }

    #[test]
    fn test_trigram_to_bytes() {
        assert_eq!(trigram_to_bytes("foo"), [b'f', b'o', b'o']);
        assert_eq!(trigram_to_bytes("ab"), [b'a', b'b', 0]);
    }

    #[test]
    fn test_bigram_to_bytes() {
        assert_eq!(bigram_to_bytes("fo"), [b'f', b'o']);
        assert_eq!(bigram_to_bytes("a"), [b'a', 0]);
    }
}
