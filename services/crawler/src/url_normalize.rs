pub fn normalize_url_advanced(input: &str) -> Option<String> {
    findverse_common::normalize_url(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_tracking_params() {
        assert_eq!(
            normalize_url_advanced("https://example.com/page?utm_source=google&id=123"),
            Some("https://example.com/page?id=123".to_string())
        );
    }

    #[test]
    fn sorts_params() {
        assert_eq!(
            normalize_url_advanced("https://example.com/?b=2&a=1"),
            Some("https://example.com/?a=1&b=2".to_string())
        );
    }

    #[test]
    fn removes_default_ports() {
        assert_eq!(
            normalize_url_advanced("https://example.com:443/page"),
            Some("https://example.com/page".to_string())
        );
    }

    #[test]
    fn removes_trailing_slash() {
        assert_eq!(
            normalize_url_advanced("https://example.com/page/"),
            Some("https://example.com/page".to_string())
        );
    }
}
