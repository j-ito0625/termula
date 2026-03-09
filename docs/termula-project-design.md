# termula — Terminal LaTeX Renderer

## プロジェクト設計書 v0.1

---

## 1. 競合調査まとめ

### 既存ツール一覧

| ツール | 言語 | 方式 | 弱点 |
|--------|------|------|------|
| **utftex** (libtexprintf) | C | LaTeX → Unicode art | ★一番完成度が高い。ただしワンショット変換のみ。ストリームフィルタ不可。画像レンダリングなし |
| **LaTerM** | TypeScript | xterm.jsアドオン + KaTeX overlay | xterm.js依存（Obsidian/VSCode専用）。ネイティブターミナルで使えない。HNで4pt/2コメント |
| **latex2sixel** | Shell | LaTeX→DVI→PNG→Sixel | TeXLive必須。Sixel専用。ストリームフィルタ不可 |
| **Terminatex** | Python | LaTeX→画像→ターミナル表示 | ワンショットのみ。Python依存。重い |
| **latex-terminal** | Python | LaTeX→画像→imgcat | iTerm2専用。ワンショット |
| **SymPy pretty** | Python | 数式→ASCII art | Python REPL内専用。汎用パイプ不可 |

### 競合の共通する弱点

1. **ストリームフィルタとして動かない** — 全員ワンショット変換のみ
2. **ターミナル非依存でない** — 特定ターミナルやフレームワークに依存
3. **graceful degradationがない** — 画像 or テキスト、どちらか片方しかサポートしない
4. **LLM CLIとの統合を想定していない** — Claude Code / aider / Copilot CLI等の文脈がない

### 最大のチャンス

**「パイプで使える汎用ストリームLaTeXレンダラ」は存在しない。**

`bat`がcatの上位互換であるように、`termula`はLaTeX混じりテキストのpretty printerとして唯一のポジションを取れる。

---

## 2. プロダクトポジショニング

### ❌ やらないこと
- 「Claude Code専用ツール」にはしない
- 「MCP Server」を主軸にしない
- ターミナルエミュレータ自体を作らない

### ✅ やること

> **termula: ターミナルのための数式レンダラ**
>
> パイプに流すだけで、LaTeXが読める数式になる。

```
# ワンショット
echo '\int_0^1 x^2 dx = \frac{1}{3}' | termula

# ストリームフィルタ（本命）
claude | termula
aider | termula
cat paper.md | termula

# ラッパー
termula -- claude
termula -- aider
```

ターゲットユーザーの優先度:
1. LLM CLIユーザー（Claude Code, aider, Copilot CLI）
2. 数学/物理/MLの研究者・学生
3. Markdown中の数式を読みたい人
4. SSHごしに数式を確認したい人

---

## 3. アーキテクチャ

```
┌─────────────────────────────────────────────────┐
│                  termula                           │
│                                                  │
│  ┌──────────┐   ┌───────────┐   ┌────────────┐  │
│  │ Scanner  │──▶│ Converter │──▶│ Renderer   │  │
│  │          │   │           │   │            │  │
│  │ stdin    │   │ LaTeX     │   │ Kitty img  │  │
│  │ stream   │   │ parser    │   │ ──fallback─│  │
│  │ から$...$│   │           │   │ Unicode art│  │
│  │ を検出   │   │ utftex    │   │ ──fallback─│  │
│  │          │   │ or KaTeX  │   │ Plain text │  │
│  └──────────┘   └───────────┘   └────────────┘  │
│                                                  │
│  ┌──────────────────────────────────────────┐    │
│  │ Terminal Detector                         │    │
│  │ $TERM / $TERM_PROGRAM / DA2 query        │    │
│  │ → Kitty / iTerm2 / Sixel / plain         │    │
│  └──────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

### 3.1 Scanner（入力パーサ）

ストリーム入力からLaTeXデリミタを検出する。

**検出対象（優先度順）:**

| パターン | 例 | 難易度 |
|----------|---|--------|
| ```` ```math ``` ```` | Markdownの数式ブロック | 低（確実） |
| `$$...$$` | display math | 中 |
| `$...$` | inline math | 高（`$HOME`等の誤検知） |
| `\[...\]` | display math (LaTeX style) | 中 |
| `\(...\)` | inline math (LaTeX style) | 中 |

**誤検知対策:**
- `$` 直後がスペース or 数字 → shell変数とみなしスキップ
- `$` の前が `\` → エスケープとみなしスキップ
- 内部にLaTeXコマンド（`\frac`, `\int`, `\sum`等）が含まれるかヒューリスティック判定
- `--delimiter` オプションで検出対象を限定可能（デフォルト: `math` ブロック + `$$` のみ）

**バッファリング戦略:**

ストリーミング入力で `$` が来た時点では開始か通常文字か判断できない。

```
方針: 短いタイムアウト + ヒューリスティック

1. `$` を検出
2. 50ms以内に次の `$` が来るか、LaTeXコマンドが来るかを待つ
3. 来なければ通常テキストとしてフラッシュ
4. 来たらLaTeXモードに入り、閉じデリミタまでバッファ

```math の場合は確実なので即座にLaTeXモードに入れる。
```

### 3.2 Converter（変換エンジン）

**推奨: utftex (libtexprintf) をライブラリとして利用**

理由:
- C言語で書かれており、FFIでRustから呼べる
- LaTeX構文の対応範囲が広い（分数、積分、行列、配列）
- Unicode art出力が高品質
- BSD-3ライセンスでOSS互換
- `brew install utftex` で既にHomebrew入り

代替案:
- **KaTeX (WASM)**: LaTeX→MathML/HTML変換。これをさらにテキスト化するのは回り道
- **SymPy**: Python依存が重すぎる
- **自前実装**: 工数が膨大。まずはutftexに乗るのが正しい

### 3.3 Renderer（出力）

ターミナルの能力に応じて3段フォールバック:

```
Level 1: Kitty Graphics Protocol（最高品質）
├── LaTeX → KaTeX → SVG → PNG → Kitty APC escape
├── 対応: Kitty, WezTerm, Ghostty
└── 品質: ◎（LaTeXと同等のレンダリング）

Level 2: Unicode Art（主力）
├── LaTeX → utftex → Unicode文字による2D描画
├── 対応: すべてのUnicode対応ターミナル
└── 品質: ○（分数・積分・行列が読みやすい）

Level 3: Inline Unicode（最低限）
├── \alpha → α, \int → ∫, \frac{a}{b} → a/b
├── 対応: すべてのターミナル
└── 品質: △（簡単な式のみ）
```

### 3.4 Terminal Detector

```rust
fn detect_terminal() -> TerminalCapability {
    // 1. 環境変数チェック
    if env("TERM_PROGRAM") == "WezTerm" { return Kitty; }
    if env("TERM") contains "kitty"     { return Kitty; }
    if env("TERM_PROGRAM") == "iTerm.app" { return ITerm2; }
    if env("TERM_PROGRAM") == "ghostty" { return Kitty; }

    // 2. DA2 (Device Attributes) query
    // → レスポンスからターミナル種別を判定

    // 3. フォールバック
    return UnicodeArt;
}
```

---

## 4. CLI設計

```
termula 0.1.0
Render LaTeX math beautifully in your terminal

USAGE:
    termula [OPTIONS] [-- <COMMAND>...]
    <stdin> | termula [OPTIONS]

ARGS:
    <COMMAND>...    Command to wrap (e.g., termula -- claude)

OPTIONS:
    -m, --mode <MODE>       Rendering mode [default: auto]
                            [possible: auto, kitty, unicode, inline, off]
    -d, --delimiters <DEL>  Delimiters to detect [default: block,display]
                            [possible: block,display,inline,all]
    -f, --font <FONT>       Font for image rendering [default: Latin Modern Math]
    --dark                  Dark background (default: auto-detect)
    --light                 Light background
    -w, --width <COLS>      Max width for Unicode art [default: terminal width]
    -v, --verbose           Show debug info
    -h, --help              Print help
    -V, --version           Print version

EXAMPLES:
    echo '\frac{a}{b}' | termula
    cat notes.md | termula
    termula -- claude
    termula -m unicode -- aider
```

---

## 5. 実装フェーズ

### Phase 1: MVP（2-3週間）— 最初のリリース

**ゴール: `echo '\frac{1}{2}' | termula` が動く**

- [ ] Rustプロジェクト初期化（clap, tokio）
- [ ] utftex の C FFI バインディング（or subprocess呼び出し）
- [ ] stdin パイプ読み取り + `$$...$$` / ` ```math ` 検出
- [ ] Unicode art 出力
- [ ] `cargo install termula` で配布
- [ ] README + before/after GIF

### Phase 2: ストリームフィルタ（2週間）

**ゴール: `claude | termula` が動く**

- [ ] pty proxy実装（ストリーミング対応）
- [ ] ANSIエスケープシーケンスの透過
- [ ] バッファリング + タイムアウトロジック
- [ ] `termula -- claude` ラッパーモード
- [ ] $...$ inline math 検出（誤検知対策込み）

### Phase 3: 画像レンダリング（2週間）

**ゴール: WezTermで美しい数式画像がインライン表示される**

- [ ] Kitty Graphics Protocol 実装
- [ ] KaTeX (Node.js) or typst でSVG生成
- [ ] SVG → PNG 変換（resvg）
- [ ] ターミナル自動検出
- [ ] iTerm2 protocol サポート
- [ ] ダーク/ライトモード対応

### Phase 4: エコシステム（継続）

- [ ] `brew install termula`
- [ ] MCP Server ラッパー（Claude Code統合用、オプション）
- [ ] Neovim / tmux 統合ガイド
- [ ] CI: GitHub Actions + cross-compile
- [ ] CLAUDE.md テンプレート配布

---

## 6. 技術的な決断ポイント

### Q1: utftex を FFI で呼ぶか、subprocess で呼ぶか？

**推奨: Phase 1 は subprocess、Phase 2以降でFFI検討**

理由: MVP を速く出すことが最重要。`utftex` コマンドがPATHにあれば subprocess で十分。
FFI はパフォーマンスが必要になってから。

### Q2: pty proxy はどう実装するか？

**推奨: `portable-pty` crate を使用**

Claude Codeはpty上で動くため、単純な pipe では ANSI エスケープや端末サイズ情報が失われる。
`portable-pty` を使って子プロセスをpty内で起動し、出力をインターセプトする。

```
termula -- claude

termula が pty を作成
  → claude を子プロセスとしてpty内で起動
  → pty の出力を読み取り
  → LaTeX パターンを検出・変換
  → 親pty（実際のターミナル）に出力
```

### Q3: 画像レンダリングのバックエンドは？

**推奨: typst（Rust native）を優先検討**

- typst: Rust製の組版エンジン。LaTeXのサブセットをネイティブにレンダリング可能。
  FFIで呼べばプロセス起動なしで高速。ただしLaTeX構文との互換性に変換層が必要。
- KaTeX (Node.js): 最も広くテスト済み。Node.js依存が痛い。
- pdflatex + dvisvgm: 最高品質だがTeXLive必須で重い。

### Q4: `$HOME` 等の誤検知をどう防ぐか？

**多段ヒューリスティック:**

```
1. `$` の直前が英数字 or `}` → LaTeX候補
2. `$` の直後がスペース → シェル変数の可能性大 → スキップ
3. 内部に `\` コマンドがあるか → あればLaTeX確定
4. 内部が純粋な英数字のみ → シェル変数の可能性大 → スキップ
5. `--delimiters block,display` をデフォルトにし、
   inline $...$ はオプトインにする
```

---

## 7. README構成案（スター獲得戦略）

```markdown
<h1 align="center">termula</h1>
<p align="center">
  <b>Beautiful math in your terminal</b>
</p>

<!-- before/after GIF (これが9割) -->

## Install

cargo install termula
brew install termula

## Quick Start

echo '\int_0^1 x^2 dx = \frac{1}{3}' | termula

## Use with AI coding tools

claude | termula
termula -- aider
termula -- claude

<!-- ターミナル別出力比較画像 -->

## How it works
1段落の説明 + アーキテクチャ図

## Terminal Support
対応ターミナル × レンダリングモードのマトリクス表
```

### 名前候補の最終評価

**決定: `termula`**（"terminal" + "formula"、7文字、造語で検索ノイズゼロ、何をするか想起しやすい）

リポジトリ名: `github.com/<username>/termula`

---

## 8. CLAUDE.md テンプレート（配布用）

termulaをインストールしたユーザーが自分のCLAUDE.mdに追加する一行:

```markdown
## Math Output
数式を出力する際は、必ず ```math ブロックで囲んでください。
インライン数式は $...$ ではなく、文章中に自然言語で記述してください。
display mathは以下の形式を使用:

\```math
\int_0^1 x^2 dx = \frac{1}{3}
\```
```

これにより termula の検出精度が最大化される。

---

## 9. 成功指標

| 期間 | 目標 |
|------|------|
| リリース1週間 | HN Show HN で50+ points |
| 1ヶ月 | GitHub 500+ stars |
| 3ヶ月 | GitHub 2000+ stars, brew公式 |
| 6ヶ月 | Claude Code / aider のドキュメントで言及 |

---

## 10. 次のアクション

1. **crates.io で `termula` の名前を確保する**
2. **Rustプロジェクト初期化 + utftex subprocess呼び出しのPoC**
3. **before/after の録画素材を作る（READMEのGIF用）**
4. **Phase 1 MVP を2-3週間で完成させてリリース**
