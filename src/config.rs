use serde::Deserialize;
use std::path::PathBuf;

/// Configuration loaded from config file.
/// All fields are optional; CLI args take precedence.
#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    pub mode: Option<String>,
    pub delimiters: Option<String>,
    pub width: Option<usize>,
    pub dark: Option<bool>,
    pub light: Option<bool>,
    pub no_cache: Option<bool>,
    pub verbose: Option<bool>,
}

/// Load config from file. Returns default if file doesn't exist.
pub fn load() -> FileConfig {
    if let Some(path) = config_path() {
        if path.exists() {
            crate::verbose_log(&format!("loading config from {}", path.display()));
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<FileConfig>(&content) {
                    Ok(cfg) => return cfg,
                    Err(e) => {
                        eprintln!("termula: warning: invalid config file: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("termula: warning: cannot read config file: {}", e);
                }
            }
        }
    }
    FileConfig::default()
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let home = std::env::var("HOME").ok()?;
            Some(PathBuf::from(home).join(".config"))
        })?;
    Some(base.join("termula").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
mode = "unicode"
delimiters = "all"
dark = true
width = 80
"#;
        let cfg: FileConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.mode.as_deref(), Some("unicode"));
        assert_eq!(cfg.delimiters.as_deref(), Some("all"));
        assert_eq!(cfg.dark, Some(true));
        assert_eq!(cfg.width, Some(80));
        assert_eq!(cfg.no_cache, None);
    }

    #[test]
    fn test_empty_config() {
        let cfg: FileConfig = toml::from_str("").unwrap();
        assert!(cfg.mode.is_none());
        assert!(cfg.delimiters.is_none());
    }
}
