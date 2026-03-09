/// Which delimiter types to detect.
#[derive(Debug, Clone)]
pub struct DelimiterConfig {
    pub block: bool,   // ```math
    pub display: bool, // $$...$$, \[...\], and \(...\)
    pub inline: bool,  // $...$
}

pub fn parse_delimiter_config(s: &str) -> DelimiterConfig {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.contains(&"all") {
        return DelimiterConfig {
            block: true,
            display: true,
            inline: true,
        };
    }
    DelimiterConfig {
        block: parts.contains(&"block"),
        display: parts.contains(&"display"),
        inline: parts.contains(&"inline"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default() {
        let config = parse_delimiter_config("block,display");
        assert!(config.block);
        assert!(config.display);
        assert!(!config.inline);
    }

    #[test]
    fn test_parse_all() {
        let config = parse_delimiter_config("all");
        assert!(config.block);
        assert!(config.display);
        assert!(config.inline);
    }

    #[test]
    fn test_parse_inline_only() {
        let config = parse_delimiter_config("inline");
        assert!(!config.block);
        assert!(!config.display);
        assert!(config.inline);
    }
}
