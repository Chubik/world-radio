pub fn is_valid_format(key: &str) -> bool {
    match key.strip_prefix("r4-") {
        None => false,
        Some(rest) => {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_key() {
        assert!(is_valid_format("r4-abc123"));
    }

    #[test]
    fn rejects_no_prefix() {
        assert!(!is_valid_format("abc123"));
    }

    #[test]
    fn rejects_empty_body() {
        assert!(!is_valid_format("r4-"));
    }

    #[test]
    fn rejects_uppercase() {
        assert!(!is_valid_format("r4-ABC"));
    }
}
