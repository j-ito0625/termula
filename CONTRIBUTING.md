# Contributing to termula

Thanks for your interest in contributing to termula!

## Development Setup

```bash
git clone https://github.com/j-ito0625/termula.git
cd termula
cargo build
```

### Dependencies

- **Rust** (stable) — the project itself
- **[utftex](https://github.com/nicokeywords/utftex)** — Unicode art rendering (`brew install utftex`). Tests that need utftex will skip if it's not installed.
- **[typst](https://typst.app/)** — Image rendering, optional (`cargo install typst-cli`)

## Development Workflow

```bash
cargo build              # build
cargo test               # run all tests
cargo clippy             # lint (must pass with no warnings)
cargo fmt                # format
```

All of these must pass before submitting a PR. CI runs them automatically on Ubuntu and macOS.

## Project Structure

```
src/
  main.rs           # CLI, event processing, rendering dispatch
  scanner.rs        # Delimiter config parsing
  stream_scanner.rs # Streaming LaTeX delimiter detection state machine
  converter.rs      # LaTeX → Unicode art (utftex) / PNG (typst) / inline Unicode
  renderer.rs       # Output: Kitty Graphics, iTerm2 inline images, Unicode art
  terminal.rs       # Terminal capability detection
  wrapper.rs        # PTY wrapper mode (termula -- cmd)
  config.rs         # Config file loading (~/.config/termula/config.toml)
tests/
  integration.rs    # End-to-end pipe and wrapper mode tests
```

## Architecture

Three-stage streaming pipeline:

```
Scanner → Converter → Renderer
```

- **Scanner** detects LaTeX delimiters in streaming text, passes through ANSI escapes
- **Converter** transforms LaTeX via utftex (Unicode art), typst+mitex (PNG), or symbol substitution (inline)
- **Renderer** outputs using Kitty Graphics protocol, iTerm2 inline images, or plain text

## Adding a Feature

1. Open an issue to discuss the feature first
2. Fork and create a feature branch
3. Write tests for new functionality
4. Ensure `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt -- --check` all pass
5. Submit a PR with a clear description

## Bug Reports

Please include:
- OS and terminal emulator
- termula version (`termula --version`)
- Steps to reproduce
- Expected vs actual behavior
- If possible, the input that triggered the bug

## Code Style

- Follow existing patterns in the codebase
- Keep functions focused and small
- Use `anyhow` for error handling
- Add tests for new functionality
- No unnecessary dependencies — keep the binary lean

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
