mod config;
mod converter;
mod renderer;
mod scanner;
mod stream_scanner;
mod terminal;
mod wrapper;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use std::io::{self, IsTerminal, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;
use stream_scanner::{Event, StreamScanner};
use terminal::Capability;

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn verbose_log(msg: &str) {
    if VERBOSE.load(Ordering::Relaxed) {
        eprintln!("[termula] {}", msg);
    }
}

#[derive(Parser)]
#[command(
    name = "termula",
    version,
    about = "Render LaTeX math beautifully in your terminal"
)]
struct Cli {
    /// Rendering mode [auto, kitty, iterm2, unicode, inline, off]
    #[arg(short, long, default_value = "auto")]
    mode: String,

    /// Delimiters to detect [block, display, inline, all]
    #[arg(short, long, default_value = "block,display")]
    delimiters: String,

    /// Max width for Unicode art (default: terminal width)
    #[arg(short, long)]
    width: Option<usize>,

    /// Force dark background rendering
    #[arg(long)]
    dark: bool,

    /// Force light background rendering
    #[arg(long)]
    light: bool,

    /// Disable image cache (for debugging)
    #[arg(long)]
    no_cache: bool,

    /// Show debug info on stderr
    #[arg(short, long)]
    verbose: bool,

    /// Generate shell completions [bash, zsh, fish, elvish, powershell]
    #[arg(long, value_name = "SHELL")]
    completions: Option<Shell>,

    /// Command to wrap (e.g., termula -- claude)
    #[arg(last = true)]
    command: Vec<String>,
}

/// Rendering configuration resolved from CLI args + auto-detection.
#[derive(Clone)]
pub struct RenderConfig {
    pub capability: Option<Capability>, // None = mode "off"
    pub dark_mode: bool,
    pub no_cache: bool,
    pub max_width: Option<usize>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        generate(shell, &mut cmd, "termula", &mut io::stdout());
        return Ok(());
    }

    // Load config file (CLI args take precedence)
    let file_config = config::load();

    let verbose = cli.verbose || file_config.verbose.unwrap_or(false);
    VERBOSE.store(verbose, Ordering::Relaxed);

    let delimiters_str = if cli.delimiters != "block,display" {
        &cli.delimiters
    } else {
        file_config.delimiters.as_deref().unwrap_or(&cli.delimiters)
    };
    let delimiters = scanner::parse_delimiter_config(delimiters_str);
    let render_config = resolve_render_config(&cli, &file_config);

    verbose_log(&format!(
        "mode={}, capability={:?}, dark={}, delimiters={}",
        cli.mode, render_config.capability, render_config.dark_mode, delimiters_str
    ));

    if !cli.command.is_empty() {
        let exit_code = wrapper::run(&cli.command, &delimiters, &render_config)?;
        std::process::exit(exit_code);
    }

    // Pipe filter mode
    if std::io::stdin().is_terminal() {
        eprintln!("termula: reading from stdin (pipe some input or use -- to wrap a command)");
    }

    let mut scanner = StreamScanner::new(delimiters);
    let mut stdout = io::stdout();

    // Channel-based stdin reading so $ timeout works even when stdin blocks.
    let (tx, rx) = mpsc::sync_channel::<Option<Vec<u8>>>(16);
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut stdin_lock = stdin.lock();
        let mut buf = [0u8; 4096];
        loop {
            match stdin_lock.read(&mut buf) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(n) => {
                    if tx.send(Some(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });

    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Some(data)) => {
                let chunk = String::from_utf8_lossy(&data);
                let events = scanner.feed(&chunk);
                process_events(&events, &mut stdout, &render_config);

                let timeout_events = scanner.check_timeout();
                process_events(&timeout_events, &mut stdout, &render_config);
            }
            Ok(None) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let timeout_events = scanner.check_timeout();
                process_events(&timeout_events, &mut stdout, &render_config);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let final_events = scanner.flush();
    process_events(&final_events, &mut stdout, &render_config);

    Ok(())
}

fn resolve_render_config(cli: &Cli, file_config: &config::FileConfig) -> RenderConfig {
    let mode = if cli.mode != "auto" {
        &cli.mode
    } else {
        file_config.mode.as_deref().unwrap_or(&cli.mode)
    };

    let capability = if mode == "off" {
        None
    } else {
        Some(terminal::parse_mode(mode).unwrap_or_else(terminal::detect))
    };

    let dark_mode = if cli.dark {
        true
    } else if cli.light {
        false
    } else if let Some(dark) = file_config.dark {
        dark
    } else if let Some(true) = file_config.light {
        false
    } else {
        terminal::is_dark_mode()
    };

    let no_cache = cli.no_cache || file_config.no_cache.unwrap_or(false);
    let max_width = cli.width.or(file_config.width);

    RenderConfig {
        capability,
        dark_mode,
        no_cache,
        max_width,
    }
}

pub fn process_events(events: &[Event], stdout: &mut impl Write, config: &RenderConfig) {
    for event in events {
        match event {
            Event::Text(text) => {
                let _ = stdout.write_all(text.as_bytes());
                let _ = stdout.flush();
            }
            Event::Math(latex) => {
                render_math(latex, stdout, config);
            }
        }
    }
}

fn render_math(latex: &str, stdout: &mut impl Write, config: &RenderConfig) {
    let cap = match &config.capability {
        Some(c) => c,
        None => {
            let _ = write!(stdout, "$${}$$", latex);
            let _ = stdout.flush();
            return;
        }
    };

    verbose_log(&format!(
        "rendering math ({:?}): {}",
        cap,
        if latex.len() > 40 {
            &latex[..40]
        } else {
            latex
        }
    ));

    match cap {
        Capability::KittyGraphics | Capability::ITerm2 => {
            match converter::convert_to_image(latex, config.dark_mode, config.no_cache) {
                Ok(png) => {
                    verbose_log(&format!("image rendered: {} bytes", png.len()));
                    if *cap == Capability::KittyGraphics {
                        renderer::render_kitty_image(&png);
                    } else {
                        renderer::render_iterm2_image(&png);
                    }
                }
                Err(e) => {
                    verbose_log(&format!(
                        "image rendering failed, falling back to unicode: {}",
                        e
                    ));
                    render_unicode_fallback(latex, stdout, config.max_width);
                }
            }
        }
        Capability::UnicodeArt => {
            render_unicode_fallback(latex, stdout, config.max_width);
        }
        Capability::InlineUnicode => {
            let rendered = converter::convert_inline(latex);
            let _ = write!(stdout, "{}", rendered);
            let _ = stdout.flush();
        }
    }
}

fn render_unicode_fallback(latex: &str, stdout: &mut impl Write, max_width: Option<usize>) {
    match converter::convert(latex, max_width) {
        Ok(rendered) => {
            renderer::render_unicode(&rendered);
        }
        Err(e) => {
            verbose_log(&format!(
                "utftex failed, falling back to inline unicode: {}",
                e
            ));
            let rendered = converter::convert_inline(latex);
            let _ = write!(stdout, "{}", rendered);
            let _ = stdout.flush();
        }
    }
}
