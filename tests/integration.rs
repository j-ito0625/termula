use std::io::Write;
use std::process::{Command, Stdio};

fn termula_pipe(input: &str, args: &[&str]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_termula"));
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn termula");
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("failed to wait");
    assert!(output.status.success(), "termula exited with error");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_plain_text_passthrough() {
    let out = termula_pipe("hello world\n", &["-m", "off"]);
    assert_eq!(out, "hello world\n");
}

#[test]
fn test_mode_off_preserves_dollar_dollar() {
    let out = termula_pipe("before $$x^2$$ after\n", &["-m", "off"]);
    assert_eq!(out, "before $$x^2$$ after\n");
}

#[test]
fn test_unicode_mode_renders_math() {
    // In unicode mode with utftex available, math should be rendered (not raw LaTeX)
    let out = termula_pipe("$$x^2$$", &["-m", "unicode"]);
    // The output should NOT contain the raw $$ delimiters
    assert!(
        !out.contains("$$"),
        "math should be rendered, not raw: {}",
        out
    );
}

#[test]
fn test_math_block_detection() {
    let input = "before\n```math\nx^2\n```\nafter";
    let out = termula_pipe(input, &["-m", "unicode"]);
    // Should not contain the ```math markers
    assert!(
        !out.contains("```math"),
        "math block should be consumed: {}",
        out
    );
    assert!(out.contains("before"));
    assert!(out.contains("after"));
}

#[test]
fn test_bracket_math() {
    let out = termula_pipe("see \\[x^2\\] here", &["-m", "unicode"]);
    assert!(
        !out.contains("\\["),
        "bracket math should be rendered: {}",
        out
    );
    assert!(out.contains("see"));
    assert!(out.contains("here"));
}

#[test]
fn test_paren_math_default_on() {
    // \(...\) should be detected by default (it's in the display group now)
    let out = termula_pipe("see \\(x^2\\) here", &["-m", "unicode"]);
    assert!(
        !out.contains("\\("),
        "paren math should be rendered by default: {}",
        out
    );
}

#[test]
fn test_inline_dollar_off_by_default() {
    // $...$ should NOT be detected by default
    let out = termula_pipe("val $\\alpha$ end", &["-m", "unicode"]);
    assert!(
        out.contains("$\\alpha$") || out.contains("$"),
        "inline $ should pass through by default"
    );
}

#[test]
fn test_inline_unicode_mode() {
    let out = termula_pipe("$$\\alpha + \\beta$$", &["-m", "inline"]);
    assert!(out.contains("α"), "should contain alpha symbol: {}", out);
    assert!(out.contains("β"), "should contain beta symbol: {}", out);
}

#[test]
fn test_no_cache_flag_accepted() {
    // Just verify the flag doesn't cause an error
    let out = termula_pipe("hello", &["-m", "off", "--no-cache"]);
    assert_eq!(out, "hello");
}

#[test]
fn test_multiple_math_blocks() {
    let input = "a $$x^2$$ b $$y^3$$ c";
    let out = termula_pipe(input, &["-m", "unicode"]);
    assert!(
        !out.contains("$$"),
        "all math blocks should be rendered: {}",
        out
    );
    assert!(out.contains("a "));
    assert!(out.contains(" c"));
}

#[test]
fn test_ansi_escapes_passthrough() {
    let input = "hello \x1b[31mred\x1b[0m world";
    let out = termula_pipe(input, &["-m", "off"]);
    assert_eq!(out, input);
}

#[test]
fn test_version_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_termula"))
        .arg("--version")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("termula"));
}

#[test]
fn test_help_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_termula"))
        .arg("--help")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Render LaTeX"));
}

#[test]
fn test_completions_bash() {
    let output = Command::new(env!("CARGO_BIN_EXE_termula"))
        .args(["--completions", "bash"])
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("termula"),
        "bash completion should reference termula"
    );
}

#[test]
fn test_wrapper_echo_passthrough() {
    // termula -- echo "hello" should pass through plain text
    let output = Command::new(env!("CARGO_BIN_EXE_termula"))
        .args(["-m", "off", "--", "echo", "hello world"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello world"),
        "wrapper should pass through echo output: {:?}",
        stdout
    );
}

#[test]
fn test_wrapper_exit_code() {
    // termula -- false should exit with non-zero
    let output = Command::new(env!("CARGO_BIN_EXE_termula"))
        .args(["-m", "off", "--", "false"])
        .output()
        .expect("failed to run");
    assert!(
        !output.status.success(),
        "should propagate child's non-zero exit code"
    );
}

#[test]
fn test_wrapper_printf_math() {
    // Wrapper mode with inline unicode rendering
    // Use printf %s to avoid \alpha being interpreted as \a (BEL)
    let output = Command::new(env!("CARGO_BIN_EXE_termula"))
        .args(["-m", "inline", "--", "printf", "%s", "$$\\alpha + \\beta$$"])
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains('α') || stdout.contains('β'),
        "wrapper should render math inline: {:?}",
        stdout
    );
}
