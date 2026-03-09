# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**termula** — a terminal LaTeX renderer. Pipe-friendly stream filter that detects LaTeX in stdin and renders it as Unicode art or inline images.

Primary use case: `claude | termula` / `termula -- aider` — making math readable in LLM CLI output.

Language: **Rust**. Key crates: `clap` (CLI), `tokio` (async), `portable-pty` (pty proxy for wrapper mode).

## Build Commands

```bash
cargo build              # build
cargo test               # run all tests
cargo test <test_name>   # run a single test
cargo clippy             # lint
cargo fmt                # format
cargo install --path .   # install locally
```

## Architecture

Three-stage pipeline processing stdin as a stream:

```
Scanner → Converter → Renderer
```

1. **Scanner** — detects LaTeX delimiters in streaming text (` ```math `, `$$...$$`, `\[...\]`, `\(...\)`, `$...$`). Uses timeout-based buffering (50ms) for ambiguous `$` delimiters. Default detection: block + display math only; inline `$...$` is opt-in via `--delimiters`.

2. **Converter** — transforms LaTeX to renderable output. Phase 1 uses `utftex` as a subprocess; later phases may use C FFI for performance.

3. **Renderer** — three-level fallback based on terminal capability:
   - **Level 1 (Kitty Graphics)**: LaTeX → SVG → PNG → Kitty APC escape (Kitty, WezTerm, Ghostty)
   - **Level 2 (Unicode Art)**: via utftex (any Unicode terminal) — the primary output mode
   - **Level 3 (Inline Unicode)**: symbol substitution only (`\alpha` → `α`, `\frac{a}{b}` → `a/b`)

4. **Terminal Detector** — probes `$TERM`, `$TERM_PROGRAM`, and DA2 queries to determine capability level.

## Two Operating Modes

- **Pipe filter**: `<stdin> | termula` — read stdin, detect/render LaTeX, write to stdout
- **Wrapper**: `termula -- <cmd>` — spawn child process in a pty via `portable-pty`, intercept output, pass through ANSI escapes while transforming LaTeX

## Key Design Decisions

- `$...$` inline math detection defaults to **off** (too many false positives with shell variables like `$HOME`). Users opt in with `--delimiters all`.
- Multi-heuristic false-positive prevention: check for `\` commands inside delimiters, skip `$` followed by space/digit, skip escaped `\$`.
- Phase 1 calls `utftex` as subprocess (speed to MVP); FFI binding planned for Phase 2+.
- Image rendering backend: prefer **typst** (Rust-native) over KaTeX (Node.js dependency) or pdflatex (TeXLive dependency).

## Implementation Phases

- **Phase 1 (MVP)**: pipe mode + `$$`/` ```math ` detection + utftex subprocess + Unicode art output
- **Phase 2**: stream filter with pty proxy, ANSI passthrough, wrapper mode, inline `$` detection
- **Phase 3**: Kitty/iTerm2 image rendering, terminal auto-detection, dark/light mode
- **Phase 4**: Homebrew formula, cross-compilation CI, ecosystem integrations
