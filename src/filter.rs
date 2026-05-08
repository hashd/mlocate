use crate::error::MlocateError;

#[derive(Debug, Clone)]
pub struct SizeFilter {
    pub bytes: u64,
    pub operator: CmpOp,
}

#[derive(Debug, Clone)]
pub struct ModifiedFilter {
    pub seconds: i64,
    pub operator: CmpOp,
}

#[derive(Debug, Clone)]
pub struct MimeFilter {
    pub pattern: String,
    pub is_glob: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CmpOp {
    Eq,
    Ge,
    Le,
}

pub fn parse_size(input: &str) -> Result<SizeFilter, MlocateError> {
    let input = input.trim();
    let (value_part, op) = if let Some(stripped) = input.strip_suffix('+') {
        (stripped, CmpOp::Ge)
    } else if let Some(stripped) = input.strip_suffix('-') {
        (stripped, CmpOp::Le)
    } else {
        (input, CmpOp::Eq)
    };

    let bytes = parse_bytes(value_part)?;
    Ok(SizeFilter { bytes, operator: op })
}

fn parse_bytes(input: &str) -> Result<u64, MlocateError> {
    let input = input.trim().to_uppercase();
    if let Some(num_str) = input.strip_suffix("GB") {
        return num_str.trim().parse::<u64>()
            .map(|n| n * 1_000_000_000)
            .map_err(|_| MlocateError::InvalidSizeFilter { input: input.to_string() });
    }
    if let Some(num_str) = input.strip_suffix("MB") {
        return num_str.trim().parse::<u64>()
            .map(|n| n * 1_000_000)
            .map_err(|_| MlocateError::InvalidSizeFilter { input: input.to_string() });
    }
    if let Some(num_str) = input.strip_suffix("KB") {
        return num_str.trim().parse::<u64>()
            .map(|n| n * 1_000)
            .map_err(|_| MlocateError::InvalidSizeFilter { input: input.to_string() });
    }
    if let Some(num_str) = input.strip_suffix('B') {
        return num_str.trim().parse::<u64>()
            .map_err(|_| MlocateError::InvalidSizeFilter { input: input.to_string() });
    }
    input.trim().parse::<u64>()
        .map_err(|_| MlocateError::InvalidSizeFilter { input: input.to_string() })
}

pub fn parse_modified(input: &str) -> Result<ModifiedFilter, MlocateError> {
    let input = input.trim();
    let (value_part, op) = if let Some(stripped) = input.strip_suffix('+') {
        (stripped, CmpOp::Le)
    } else if let Some(stripped) = input.strip_suffix('-') {
        (stripped, CmpOp::Ge)
    } else {
        (input, CmpOp::Eq)
    };

    let seconds = parse_duration(value_part)?;
    Ok(ModifiedFilter { seconds, operator: op })
}

fn parse_duration(input: &str) -> Result<i64, MlocateError> {
    let input = input.trim();
    if let Some(num_str) = input.strip_suffix('w') {
        return num_str.parse::<i64>()
            .map(|n| n * 7 * 24 * 3600)
            .map_err(|_| MlocateError::InvalidTimeFilter { input: input.to_string() });
    }
    if let Some(num_str) = input.strip_suffix('d') {
        return num_str.parse::<i64>()
            .map(|n| n * 24 * 3600)
            .map_err(|_| MlocateError::InvalidTimeFilter { input: input.to_string() });
    }
    if let Some(num_str) = input.strip_suffix('h') {
        return num_str.parse::<i64>()
            .map(|n| n * 3600)
            .map_err(|_| MlocateError::InvalidTimeFilter { input: input.to_string() });
    }
    if let Some(num_str) = input.strip_suffix('m') {
        return num_str.parse::<i64>()
            .map(|n| n * 60)
            .map_err(|_| MlocateError::InvalidTimeFilter { input: input.to_string() });
    }
    Err(MlocateError::InvalidTimeFilter { input: input.to_string() })
}

pub fn parse_mime_type(input: &str) -> Result<MimeFilter, MlocateError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(MlocateError::InvalidMimeType { input: input.to_string() });
    }
    if input.contains('*') {
        if !input.contains('/') || input.chars().filter(|&c| c == '*').count() > 1 {
            return Err(MlocateError::InvalidMimeType { input: input.to_string() });
        }
        Ok(MimeFilter { pattern: input.to_string(), is_glob: true })
    } else {
        if !input.contains('/') {
            return Err(MlocateError::InvalidMimeType { input: input.to_string() });
        }
        Ok(MimeFilter { pattern: input.to_string(), is_glob: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        let f = parse_size("10MB+").unwrap();
        assert_eq!(f.bytes, 10_000_000);
        assert_eq!(f.operator, CmpOp::Ge);

        let f = parse_size("1KB-").unwrap();
        assert_eq!(f.bytes, 1_000);
        assert_eq!(f.operator, CmpOp::Le);

        let f = parse_size("500MB").unwrap();
        assert_eq!(f.bytes, 500_000_000);
        assert_eq!(f.operator, CmpOp::Eq);

        let f = parse_size("2GB+").unwrap();
        assert_eq!(f.bytes, 2_000_000_000);
    }

    #[test]
    fn test_size_filter_invalid() {
        assert!(parse_size("abc").is_err());
        assert!(parse_size("").is_err());
    }

    #[test]
    fn test_parse_modified() {
        let f = parse_modified("2d-").unwrap();
        assert_eq!(f.seconds, 2 * 24 * 3600);
        assert_eq!(f.operator, CmpOp::Ge);

        let f = parse_modified("1w+").unwrap();
        assert_eq!(f.seconds, 7 * 24 * 3600);
        assert_eq!(f.operator, CmpOp::Le);

        let f = parse_modified("30m").unwrap();
        assert_eq!(f.seconds, 30 * 60);
    }

    #[test]
    fn test_parse_mime_type() {
        let f = parse_mime_type("text/plain").unwrap();
        assert!(!f.is_glob);
        assert_eq!(f.pattern, "text/plain");

        let f = parse_mime_type("image/*").unwrap();
        assert!(f.is_glob);
        assert_eq!(f.pattern, "image/*");
    }

    #[test]
    fn test_mime_type_invalid() {
        assert!(parse_mime_type("text").is_err());
        assert!(parse_mime_type("").is_err());
        assert!(parse_mime_type("*/*").is_err());
    }
}
