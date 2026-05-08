pub fn generate_trigrams(path: &str) -> Vec<String> {
    if path.is_empty() {
        return vec!["___".to_string()];
    }
    let s = if path.len() < 3 {
        let padded = format!("_{}_", path);
        padded.chars().take(3).collect::<String>()
    } else {
        path.to_string()
    };

    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        return vec![s];
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
    let lowered: String = path.chars().map(|c| c.to_ascii_lowercase()).collect();
    generate_trigrams(&lowered)
}

pub fn trigram_to_bytes(trigram: &str) -> [u8; 3] {
    let bytes = trigram.as_bytes();
    let mut arr = [0u8; 3];
    let len = bytes.len().min(3);
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
    fn test_trigram_to_bytes() {
        assert_eq!(trigram_to_bytes("foo"), [b'f', b'o', b'o']);
        assert_eq!(trigram_to_bytes("ab"), [b'a', b'b', 0]);
    }
}
