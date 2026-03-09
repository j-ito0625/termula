/// Terminal rendering capability levels.
#[derive(Debug, Clone, PartialEq)]
pub enum Capability {
    KittyGraphics,
    ITerm2,
    UnicodeArt,
    InlineUnicode,
}

/// Detect the current terminal's rendering capability.
pub fn detect() -> Capability {
    // Check TERM_PROGRAM first (most reliable)
    if let Ok(prog) = std::env::var("TERM_PROGRAM") {
        match prog.as_str() {
            "WezTerm" | "ghostty" => return Capability::KittyGraphics,
            "iTerm.app" => return Capability::ITerm2,
            _ => {}
        }
    }

    if let Ok(term) = std::env::var("TERM") {
        if term.contains("kitty") {
            return Capability::KittyGraphics;
        }
    }

    // KITTY_WINDOW_ID is set inside kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return Capability::KittyGraphics;
    }

    // ITERM_SESSION_ID is set inside iTerm2
    if std::env::var("ITERM_SESSION_ID").is_ok() {
        return Capability::ITerm2;
    }

    Capability::UnicodeArt
}

/// Parse --mode flag to a capability. Returns None for "auto".
pub fn parse_mode(mode: &str) -> Option<Capability> {
    match mode {
        "kitty" => Some(Capability::KittyGraphics),
        "iterm2" => Some(Capability::ITerm2),
        "unicode" => Some(Capability::UnicodeArt),
        "inline" => Some(Capability::InlineUnicode),
        _ => None,
    }
}

/// Detect if the terminal is using a dark background.
pub fn is_dark_mode() -> bool {
    // COLORFGBG: "fg;bg" — bg < 8 is typically dark
    if let Ok(val) = std::env::var("COLORFGBG") {
        if let Some(bg) = val.rsplit(';').next() {
            if let Ok(n) = bg.parse::<u8>() {
                return n < 8;
            }
        }
    }
    // Default: assume dark background (most common for developers)
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mode() {
        assert_eq!(parse_mode("kitty"), Some(Capability::KittyGraphics));
        assert_eq!(parse_mode("iterm2"), Some(Capability::ITerm2));
        assert_eq!(parse_mode("unicode"), Some(Capability::UnicodeArt));
        assert_eq!(parse_mode("inline"), Some(Capability::InlineUnicode));
        assert_eq!(parse_mode("auto"), None);
        assert_eq!(parse_mode("off"), None);
    }
}
