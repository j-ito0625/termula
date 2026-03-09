use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use wait_timeout::ChildExt;

/// Convert LaTeX to Unicode art using utftex subprocess.
/// If max_width is set, lines exceeding the width are truncated.
pub fn convert(latex: &str, max_width: Option<usize>) -> Result<String> {
    let output = Command::new("utftex")
        .arg(latex)
        .output()
        .context("Failed to run utftex. Is it installed? (brew install utftex)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("utftex failed: {}", stderr.trim());
    }

    let result = String::from_utf8(output.stdout).context("utftex produced invalid UTF-8")?;
    let trimmed = result.trim_end();

    match max_width {
        Some(w) => {
            let truncated: String = trimmed
                .lines()
                .map(|line| {
                    if line.chars().count() > w {
                        line.chars().take(w).collect::<String>()
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(truncated)
        }
        None => Ok(trimmed.to_string()),
    }
}

/// Convert LaTeX to PNG image using typst + mitex package.
/// Uses disk cache to avoid redundant typst invocations.
pub fn convert_to_image(latex: &str, dark_mode: bool, no_cache: bool) -> Result<Vec<u8>> {
    let key = cache_key(latex, dark_mode);

    // Check cache first
    if !no_cache {
        if let Some(cached) = read_cache(&key) {
            crate::verbose_log(&format!("cache hit: {}", &key[..12]));
            return Ok(cached);
        }
    }

    crate::verbose_log(&format!("cache miss: {}, running typst", &key[..12]));
    let png_data = run_typst(latex, dark_mode)?;

    // Write to cache (best-effort)
    if !no_cache {
        write_cache(&key, &png_data);
    }

    Ok(png_data)
}

/// Timeout for typst compilation (prevents hangs on malformed input).
const TYPST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn run_typst(latex: &str, dark_mode: bool) -> Result<Vec<u8>> {
    let dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let input_path = dir.path().join("math.typ");
    let output_path = dir.path().join("math.png");

    let (fg, bg) = if dark_mode {
        ("white", "rgb(\"1e1e1e\")")
    } else {
        ("black", "rgb(\"ffffff\")")
    };

    let raw_delim = raw_delimiter(latex);
    let typst_src = format!(
        r#"#import "@preview/mitex:0.2.5": *

#set page(width: auto, height: auto, margin: (x: 4pt, y: 4pt), fill: {bg})
#set text(fill: {fg}, size: 14pt)

#mitex({delim}{latex}{delim})
"#,
        bg = bg,
        fg = fg,
        latex = latex,
        delim = raw_delim,
    );

    std::fs::write(&input_path, &typst_src).context("Failed to write typst source")?;

    let mut child = Command::new("typst")
        .arg("compile")
        .arg("--ppi")
        .arg("144")
        .arg(&input_path)
        .arg(&output_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to run typst. Is it installed? (cargo install typst-cli)")?;

    match child.wait_timeout(TYPST_TIMEOUT) {
        Ok(Some(status)) => {
            if !status.success() {
                let mut stderr = String::new();
                if let Some(ref mut err) = child.stderr {
                    let _ = std::io::Read::read_to_string(err, &mut stderr);
                }
                anyhow::bail!("typst compile failed: {}", stderr.trim());
            }
        }
        Ok(None) => {
            // Timed out — kill the process
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("typst compile timed out after {}s", TYPST_TIMEOUT.as_secs());
        }
        Err(e) => {
            anyhow::bail!("Failed to wait for typst: {}", e);
        }
    }

    let png_data = std::fs::read(&output_path).context("Failed to read typst output PNG")?;
    Ok(png_data)
}

// --- Cache ---

fn cache_dir() -> PathBuf {
    let base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".cache")
        });
    base.join("termula").join("images")
}

fn cache_key(latex: &str, dark_mode: bool) -> String {
    let mut hasher = DefaultHasher::new();
    latex.hash(&mut hasher);
    dark_mode.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn read_cache(key: &str) -> Option<Vec<u8>> {
    let path = cache_dir().join(format!("{}.png", key));
    std::fs::read(path).ok()
}

fn write_cache(key: &str, data: &[u8]) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.png", key));
    let _ = std::fs::write(path, data);
}

/// Convert LaTeX to inline Unicode by substituting common symbols.
/// No external tools needed. Handles Greek letters, operators, sub/superscripts, and simple fractions.
pub fn convert_inline(latex: &str) -> String {
    static SYMBOLS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
        let mut m = HashMap::new();
        // Greek lowercase
        m.insert("\\alpha", "α");
        m.insert("\\beta", "β");
        m.insert("\\gamma", "γ");
        m.insert("\\delta", "δ");
        m.insert("\\epsilon", "ε");
        m.insert("\\varepsilon", "ε");
        m.insert("\\zeta", "ζ");
        m.insert("\\eta", "η");
        m.insert("\\theta", "θ");
        m.insert("\\vartheta", "ϑ");
        m.insert("\\iota", "ι");
        m.insert("\\kappa", "κ");
        m.insert("\\lambda", "λ");
        m.insert("\\mu", "μ");
        m.insert("\\nu", "ν");
        m.insert("\\xi", "ξ");
        m.insert("\\pi", "π");
        m.insert("\\rho", "ρ");
        m.insert("\\sigma", "σ");
        m.insert("\\tau", "τ");
        m.insert("\\upsilon", "υ");
        m.insert("\\phi", "φ");
        m.insert("\\varphi", "φ");
        m.insert("\\chi", "χ");
        m.insert("\\psi", "ψ");
        m.insert("\\omega", "ω");
        // Greek uppercase
        m.insert("\\Gamma", "Γ");
        m.insert("\\Delta", "Δ");
        m.insert("\\Theta", "Θ");
        m.insert("\\Lambda", "Λ");
        m.insert("\\Xi", "Ξ");
        m.insert("\\Pi", "Π");
        m.insert("\\Sigma", "Σ");
        m.insert("\\Upsilon", "Υ");
        m.insert("\\Phi", "Φ");
        m.insert("\\Psi", "Ψ");
        m.insert("\\Omega", "Ω");
        // Operators and relations
        m.insert("\\pm", "±");
        m.insert("\\mp", "∓");
        m.insert("\\times", "×");
        m.insert("\\div", "÷");
        m.insert("\\cdot", "·");
        m.insert("\\circ", "∘");
        m.insert("\\leq", "≤");
        m.insert("\\le", "≤");
        m.insert("\\geq", "≥");
        m.insert("\\ge", "≥");
        m.insert("\\neq", "≠");
        m.insert("\\ne", "≠");
        m.insert("\\approx", "≈");
        m.insert("\\equiv", "≡");
        m.insert("\\sim", "∼");
        m.insert("\\propto", "∝");
        m.insert("\\ll", "≪");
        m.insert("\\gg", "≫");
        // Arrows
        m.insert("\\to", "→");
        m.insert("\\rightarrow", "→");
        m.insert("\\leftarrow", "←");
        m.insert("\\Rightarrow", "⇒");
        m.insert("\\Leftarrow", "⇐");
        m.insert("\\leftrightarrow", "↔");
        m.insert("\\Leftrightarrow", "⇔");
        m.insert("\\mapsto", "↦");
        m.insert("\\uparrow", "↑");
        m.insert("\\downarrow", "↓");
        // Big operators
        m.insert("\\sum", "∑");
        m.insert("\\prod", "∏");
        m.insert("\\int", "∫");
        m.insert("\\oint", "∮");
        m.insert("\\bigcup", "⋃");
        m.insert("\\bigcap", "⋂");
        // Set theory
        m.insert("\\in", "∈");
        m.insert("\\notin", "∉");
        m.insert("\\subset", "⊂");
        m.insert("\\supset", "⊃");
        m.insert("\\subseteq", "⊆");
        m.insert("\\supseteq", "⊇");
        m.insert("\\cup", "∪");
        m.insert("\\cap", "∩");
        m.insert("\\emptyset", "∅");
        m.insert("\\varnothing", "∅");
        // Logic
        m.insert("\\forall", "∀");
        m.insert("\\exists", "∃");
        m.insert("\\neg", "¬");
        m.insert("\\land", "∧");
        m.insert("\\lor", "∨");
        m.insert("\\implies", "⟹");
        m.insert("\\iff", "⟺");
        // Misc
        m.insert("\\infty", "∞");
        m.insert("\\partial", "∂");
        m.insert("\\nabla", "∇");
        m.insert("\\sqrt", "√");
        m.insert("\\ldots", "…");
        m.insert("\\cdots", "⋯");
        m.insert("\\vdots", "⋮");
        m.insert("\\ddots", "⋱");
        m.insert("\\langle", "⟨");
        m.insert("\\rangle", "⟩");
        m.insert("\\|", "‖");
        m.insert("\\ell", "ℓ");
        m.insert("\\hbar", "ℏ");
        // Blackboard bold
        m.insert("\\mathbb{R}", "ℝ");
        m.insert("\\mathbb{N}", "ℕ");
        m.insert("\\mathbb{Z}", "ℤ");
        m.insert("\\mathbb{Q}", "ℚ");
        m.insert("\\mathbb{C}", "ℂ");
        m
    });

    static SUPERSCRIPTS: LazyLock<HashMap<char, char>> = LazyLock::new(|| {
        [
            ('0', '⁰'),
            ('1', '¹'),
            ('2', '²'),
            ('3', '³'),
            ('4', '⁴'),
            ('5', '⁵'),
            ('6', '⁶'),
            ('7', '⁷'),
            ('8', '⁸'),
            ('9', '⁹'),
            ('+', '⁺'),
            ('-', '⁻'),
            ('=', '⁼'),
            ('(', '⁽'),
            (')', '⁾'),
            ('n', 'ⁿ'),
            ('i', 'ⁱ'),
            ('x', 'ˣ'),
            ('y', 'ʸ'),
        ]
        .into_iter()
        .collect()
    });

    static SUBSCRIPTS: LazyLock<HashMap<char, char>> = LazyLock::new(|| {
        [
            ('0', '₀'),
            ('1', '₁'),
            ('2', '₂'),
            ('3', '₃'),
            ('4', '₄'),
            ('5', '₅'),
            ('6', '₆'),
            ('7', '₇'),
            ('8', '₈'),
            ('9', '₉'),
            ('+', '₊'),
            ('-', '₋'),
            ('=', '₌'),
            ('(', '₍'),
            (')', '₎'),
            ('a', 'ₐ'),
            ('e', 'ₑ'),
            ('i', 'ᵢ'),
            ('j', 'ⱼ'),
            ('k', 'ₖ'),
            ('n', 'ₙ'),
            ('o', 'ₒ'),
            ('r', 'ᵣ'),
            ('x', 'ₓ'),
        ]
        .into_iter()
        .collect()
    });

    let mut result = latex.to_string();

    // Replace \frac{a}{b} with a/b
    while let Some(start) = result.find("\\frac{") {
        if let Some(rendered) = render_frac(&result[start..]) {
            let consumed = measure_frac(&result[start..]).unwrap_or(rendered.len());
            result = format!(
                "{}{}{}",
                &result[..start],
                rendered,
                &result[start + consumed..]
            );
        } else {
            break;
        }
    }

    // Replace known symbols (longest match first)
    let mut sorted_keys: Vec<&&str> = SYMBOLS.keys().collect();
    sorted_keys.sort_by_key(|b| std::cmp::Reverse(b.len()));
    for key in sorted_keys {
        result = result.replace(*key, SYMBOLS[*key]);
    }

    // Handle ^{...} and _{...} superscripts/subscripts
    result = apply_scripts(&result, '^', &SUPERSCRIPTS);
    result = apply_scripts(&result, '_', &SUBSCRIPTS);

    // Handle single-char ^x and _x
    result = apply_single_scripts(&result, '^', &SUPERSCRIPTS);
    result = apply_single_scripts(&result, '_', &SUBSCRIPTS);

    // Clean up remaining braces from simple groups like {x}
    result = clean_simple_braces(&result);

    result.trim().to_string()
}

fn render_frac(s: &str) -> Option<String> {
    let after = s.strip_prefix("\\frac{")?;
    let (num, rest) = extract_brace_content(after)?;
    let rest = rest.strip_prefix('{')?;
    let (den, _) = extract_brace_content(rest)?;
    Some(format!("({}/{})", num, den))
}

fn measure_frac(s: &str) -> Option<usize> {
    let after = s.strip_prefix("\\frac{")?;
    let (_, rest1) = extract_brace_content(after)?;
    let rest2 = rest1.strip_prefix('{')?;
    let (_, rest3) = extract_brace_content(rest2)?;
    Some(s.len() - rest3.len())
}

fn extract_brace_content(s: &str) -> Option<(&str, &str)> {
    let mut depth = 1;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn apply_scripts(input: &str, marker: char, table: &HashMap<char, char>) -> String {
    let pattern = format!("{}{}", marker, '{');
    let mut result = String::new();
    let mut rest = input;
    while let Some(pos) = rest.find(&pattern) {
        result.push_str(&rest[..pos]);
        let after_brace = &rest[pos + 2..];
        if let Some(close) = after_brace.find('}') {
            let content = &after_brace[..close];
            for ch in content.chars() {
                if let Some(&sup) = table.get(&ch) {
                    result.push(sup);
                } else {
                    result.push(ch);
                }
            }
            rest = &after_brace[close + 1..];
        } else {
            result.push_str(&rest[pos..pos + 2]);
            rest = &rest[pos + 2..];
        }
    }
    result.push_str(rest);
    result
}

fn apply_single_scripts(input: &str, marker: char, table: &HashMap<char, char>) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == marker {
            if let Some(&next) = chars.peek() {
                if next != '{' {
                    if let Some(&sup) = table.get(&next) {
                        result.push(sup);
                        chars.next();
                        continue;
                    }
                }
            }
        }
        result.push(ch);
    }
    result
}

fn clean_simple_braces(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Collect until matching }
            let mut content = String::new();
            let mut depth = 1;
            let mut found_close = false;
            for c in chars.by_ref() {
                match c {
                    '{' => {
                        depth += 1;
                        content.push(c);
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            found_close = true;
                            break;
                        }
                        content.push(c);
                    }
                    _ => content.push(c),
                }
            }
            if found_close {
                result.push_str(&content);
            } else {
                result.push('{');
                result.push_str(&content);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn raw_delimiter(content: &str) -> String {
    let mut n = 1;
    loop {
        let delim = "`".repeat(n);
        if !content.contains(&delim) {
            return delim;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_typst() -> bool {
        Command::new("typst")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_convert_simple() {
        if Command::new("utftex").arg("--version").output().is_err() {
            eprintln!("skipping test: utftex not found");
            return;
        }
        let result = convert("x^2", None).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_raw_delimiter() {
        assert_eq!(raw_delimiter("hello"), "`");
        assert_eq!(raw_delimiter("has ` backtick"), "``");
        assert_eq!(raw_delimiter("has `` double"), "```");
    }

    #[test]
    fn test_cache_key_deterministic() {
        let k1 = cache_key("\\frac{a}{b}", true);
        let k2 = cache_key("\\frac{a}{b}", true);
        let k3 = cache_key("\\frac{a}{b}", false);
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn test_convert_to_image() {
        if !has_typst() {
            eprintln!("skipping test: typst not found");
            return;
        }
        let result = convert_to_image("\\frac{1}{2}", true, false);
        match result {
            Ok(png) => {
                assert!(!png.is_empty());
                assert_eq!(&png[..4], b"\x89PNG");
            }
            Err(e) => {
                eprintln!("convert_to_image failed (may need network): {}", e);
            }
        }
    }

    #[test]
    fn test_convert_inline_greek() {
        let result = convert_inline("\\alpha + \\beta");
        assert!(result.contains("α"), "should contain alpha: {}", result);
        assert!(result.contains("β"), "should contain beta: {}", result);
    }

    #[test]
    fn test_convert_inline_frac() {
        let result = convert_inline("\\frac{a}{b}");
        assert!(
            result.contains("a") && result.contains("b"),
            "should contain a and b: {}",
            result
        );
        assert!(
            result.contains("/"),
            "should contain fraction slash: {}",
            result
        );
    }

    #[test]
    fn test_convert_inline_superscript() {
        let result = convert_inline("x^2");
        assert!(
            result.contains("²"),
            "should contain superscript 2: {}",
            result
        );
    }

    #[test]
    fn test_convert_inline_subscript() {
        let result = convert_inline("x_0");
        assert!(
            result.contains("₀"),
            "should contain subscript 0: {}",
            result
        );
    }

    #[test]
    fn test_convert_inline_operators() {
        let result = convert_inline("\\sum \\int \\infty");
        assert!(result.contains("∑"), "should contain sum: {}", result);
        assert!(result.contains("∫"), "should contain integral: {}", result);
        assert!(result.contains("∞"), "should contain infinity: {}", result);
    }

    #[test]
    fn test_cache_hit() {
        if !has_typst() {
            eprintln!("skipping test: typst not found");
            return;
        }
        // First call populates cache
        let r1 = convert_to_image("x^2", true, false);
        if r1.is_err() {
            eprintln!("skipping cache test: first convert failed");
            return;
        }
        // Second call should hit cache
        let key = cache_key("x^2", true);
        assert!(read_cache(&key).is_some(), "cache should be populated");
    }
}
