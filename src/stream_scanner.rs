use crate::scanner::DelimiterConfig;
use std::time::{Duration, Instant};

const DOLLAR_TIMEOUT: Duration = Duration::from_millis(50);

/// Strip ANSI escape sequences from a string.
/// Used to clean math content that may have ANSI codes mixed in from colored terminal output.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Consume the escape sequence
            match chars.next() {
                Some('[') => {
                    // CSI: skip until final byte (0x40-0x7E)
                    for c in chars.by_ref() {
                        if c.is_ascii() && (0x40..=0x7E).contains(&(c as u8)) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: skip until BEL or ST
                    let mut prev = ' ';
                    for c in chars.by_ref() {
                        if c == '\x07' || (c == '\\' && prev == '\x1b') {
                            break;
                        }
                        prev = c;
                    }
                }
                Some(_) => {
                    // Two-char escape, already consumed
                }
                None => {}
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Emit a Math event, stripping ANSI codes from content.
fn make_math_event(content: &str) -> Event {
    let cleaned = strip_ansi(content);
    Event::Math(cleaned.trim().to_string())
}

/// Events emitted by the streaming scanner.
#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    /// Pass-through text (including ANSI escapes).
    Text(String),
    /// Complete LaTeX math block ready for conversion.
    Math(String),
}

/// Scanner state machine for streaming input.
pub struct StreamScanner {
    config: DelimiterConfig,
    state: State,
    buffer: String,
}

#[derive(Debug)]
enum State {
    /// Normal text passthrough.
    Normal,
    /// Inside an ANSI escape sequence (ESC [ ... final byte).
    AnsiEscape,
    /// Saw a single `$`, waiting to see if it's math or shell variable.
    DollarPending { started: Instant },
    /// Saw `$$`, accumulating display math.
    DoubleDollarMath,
    /// Saw `$` confirmed as inline math start, accumulating.
    SingleDollarMath,
    /// Inside ```math block.
    MathBlock,
    /// Saw `\[`, accumulating display math.
    BracketMath,
    /// Saw `\(`, accumulating inline math.
    ParenMath,
    /// Saw `\`, waiting for next char to determine if it's `[`, `(`, or just text.
    BackslashPending,
    /// Saw one backtick, might be start of ```math.
    Backtick1,
    /// Saw two backticks.
    Backtick2,
    /// Saw three backticks, checking for "math".
    BacktickTriple,
    /// Inside closing ``` of a math block (counting backticks).
    MathBlockClosing { count: usize },
}

impl StreamScanner {
    pub fn new(config: DelimiterConfig) -> Self {
        Self {
            config,
            state: State::Normal,
            buffer: String::new(),
        }
    }

    /// Feed a chunk of input and collect emitted events.
    pub fn feed(&mut self, input: &str) -> Vec<Event> {
        let mut events = Vec::new();
        let chars = input.chars();

        for ch in chars {
            self.process_char(ch, &mut events);
        }

        events
    }

    /// Check for timeouts (call periodically). Returns events if timeout triggers.
    pub fn check_timeout(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        if let State::DollarPending { started } = &self.state {
            if started.elapsed() >= DOLLAR_TIMEOUT {
                // Timed out — emit buffered content as text
                let text = std::mem::take(&mut self.buffer);
                if !text.is_empty() {
                    events.push(Event::Text(text));
                }
                self.state = State::Normal;
            }
        }
        events
    }

    /// Flush any remaining buffered content (call at end of stream).
    pub fn flush(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        let text = std::mem::take(&mut self.buffer);
        if !text.is_empty() {
            match &self.state {
                State::Normal
                | State::DollarPending { .. }
                | State::BackslashPending
                | State::Backtick1
                | State::Backtick2
                | State::BacktickTriple => {
                    events.push(Event::Text(text));
                }
                // Unclosed math — emit as text too
                State::SingleDollarMath => {
                    events.push(Event::Text(format!("${}", text)));
                }
                State::DoubleDollarMath => {
                    events.push(Event::Text(format!("$${}", text)));
                }
                State::MathBlock => {
                    events.push(Event::Text(format!("```math\n{}", text)));
                }
                State::BracketMath => {
                    events.push(Event::Text(format!("\\[{}", text)));
                }
                State::ParenMath => {
                    events.push(Event::Text(format!("\\({}", text)));
                }
                State::AnsiEscape => {
                    events.push(Event::Text(text));
                }
                State::MathBlockClosing { .. } => {
                    events.push(Event::Text(text));
                }
            }
        }
        self.state = State::Normal;
        events
    }

    fn emit_text(buffer: &mut String, events: &mut Vec<Event>) {
        if !buffer.is_empty() {
            events.push(Event::Text(std::mem::take(buffer)));
        }
    }

    fn process_char(&mut self, ch: char, events: &mut Vec<Event>) {
        match &self.state {
            State::Normal => self.handle_normal(ch, events),
            State::AnsiEscape => self.handle_ansi_escape(ch, events),
            State::DollarPending { .. } => self.handle_dollar_pending(ch, events),
            State::DoubleDollarMath => self.handle_double_dollar_math(ch, events),
            State::SingleDollarMath => self.handle_single_dollar_math(ch, events),
            State::MathBlock => self.handle_math_block(ch, events),
            State::BracketMath => self.handle_bracket_math(ch, events),
            State::ParenMath => self.handle_paren_math(ch, events),
            State::BackslashPending => self.handle_backslash_pending(ch, events),
            State::Backtick1 => self.handle_backtick1(ch, events),
            State::Backtick2 => self.handle_backtick2(ch, events),
            State::BacktickTriple => self.handle_backtick_triple(ch, events),
            State::MathBlockClosing { .. } => self.handle_math_block_closing(ch, events),
        }
    }

    fn handle_normal(&mut self, ch: char, events: &mut Vec<Event>) {
        match ch {
            '\x1b' => {
                // Start of ANSI escape sequence
                Self::emit_text(&mut self.buffer, events);
                self.buffer.push(ch);
                self.state = State::AnsiEscape;
            }
            '$' if self.config.display || self.config.inline => {
                Self::emit_text(&mut self.buffer, events);
                self.buffer.push('$');
                self.state = State::DollarPending {
                    started: Instant::now(),
                };
            }
            '\\' if self.config.display || self.config.inline => {
                Self::emit_text(&mut self.buffer, events);
                self.buffer.push('\\');
                self.state = State::BackslashPending;
            }
            '`' if self.config.block => {
                Self::emit_text(&mut self.buffer, events);
                self.buffer.push('`');
                self.state = State::Backtick1;
            }
            _ => {
                self.buffer.push(ch);
            }
        }
    }

    fn handle_ansi_escape(&mut self, ch: char, events: &mut Vec<Event>) {
        self.buffer.push(ch);
        // ANSI CSI sequence: ESC [ ... <final byte 0x40-0x7E>
        // OSC sequence: ESC ] ... ST
        // Simple: ESC <single char>
        if self.buffer.len() == 2 {
            // Second char after ESC
            match ch {
                '[' | ']' | 'P' | 'X' | '^' | '_' => {
                    // CSI, OSC, DCS, SOS, PM, APC — multi-char sequences
                }
                _ => {
                    // Two-char escape like ESC M, ESC 7, etc.
                    events.push(Event::Text(std::mem::take(&mut self.buffer)));
                    self.state = State::Normal;
                }
            }
        } else {
            // Check for sequence termination
            let second = self.buffer.chars().nth(1).unwrap_or(' ');
            match second {
                '[' => {
                    // CSI: ends at 0x40-0x7E
                    if ch.is_ascii() && (0x40..=0x7E).contains(&(ch as u8)) {
                        events.push(Event::Text(std::mem::take(&mut self.buffer)));
                        self.state = State::Normal;
                    }
                }
                ']' => {
                    // OSC: ends at BEL (0x07) or ST (ESC \)
                    if ch == '\x07' || (ch == '\\' && self.buffer.ends_with("\x1b\\")) {
                        events.push(Event::Text(std::mem::take(&mut self.buffer)));
                        self.state = State::Normal;
                    }
                }
                _ => {
                    // DCS, APC, etc: end at ST (ESC \)
                    if ch == '\\' && self.buffer.len() >= 3 {
                        let bytes = self.buffer.as_bytes();
                        if bytes[bytes.len() - 2] == b'\x1b' {
                            events.push(Event::Text(std::mem::take(&mut self.buffer)));
                            self.state = State::Normal;
                        }
                    } else if ch == '\x07' {
                        events.push(Event::Text(std::mem::take(&mut self.buffer)));
                        self.state = State::Normal;
                    }
                }
            }
        }
    }

    fn handle_dollar_pending(&mut self, ch: char, events: &mut Vec<Event>) {
        match ch {
            '$' if self.config.display => {
                // Got $$, enter display math mode
                self.buffer.clear();
                self.state = State::DoubleDollarMath;
            }
            ' ' | '0'..='9' => {
                // Likely a shell variable like "$ " or "$1" — emit as text
                self.buffer.push(ch);
                events.push(Event::Text(std::mem::take(&mut self.buffer)));
                self.state = State::Normal;
            }
            '\\' if self.config.inline => {
                // Looks like LaTeX command inside $...$, e.g. $\alpha$
                self.buffer.clear();
                self.buffer.push(ch);
                self.state = State::SingleDollarMath;
            }
            _ if self.config.inline => {
                // Start accumulating potential inline math
                self.buffer.clear();
                self.buffer.push(ch);
                self.state = State::SingleDollarMath;
            }
            _ => {
                // Inline not enabled, emit $ + char as text
                self.buffer.push(ch);
                events.push(Event::Text(std::mem::take(&mut self.buffer)));
                self.state = State::Normal;
            }
        }
    }

    fn handle_double_dollar_math(&mut self, ch: char, events: &mut Vec<Event>) {
        if ch == '$' {
            // Could be closing $$
            // Check if next char is also $
            // For now, simple approach: if buffer ends without $, this starts closing
            if self.buffer.ends_with('$') {
                // We have $$: emit math
                self.buffer.pop(); // remove the first $
                let math = std::mem::take(&mut self.buffer);
                events.push(make_math_event(&math));
                self.state = State::Normal;
            } else {
                self.buffer.push(ch);
            }
        } else {
            self.buffer.push(ch);
        }
    }

    fn handle_single_dollar_math(&mut self, ch: char, events: &mut Vec<Event>) {
        if ch == '$' {
            let content = std::mem::take(&mut self.buffer);
            // Heuristic: only treat as math if it contains a \ command
            if content.contains('\\') {
                events.push(make_math_event(&content));
            } else {
                events.push(Event::Text(format!("${}$", content)));
            }
            self.state = State::Normal;
        } else {
            self.buffer.push(ch);
        }
    }

    fn handle_math_block(&mut self, ch: char, _events: &mut [Event]) {
        if ch == '`' {
            // Potential start of closing ```
            self.state = State::MathBlockClosing { count: 1 };
        } else {
            self.buffer.push(ch);
        }
    }

    fn handle_math_block_closing(&mut self, ch: char, events: &mut Vec<Event>) {
        if let State::MathBlockClosing { count } = self.state {
            if ch == '`' {
                if count + 1 >= 3 {
                    // Got closing ```
                    let math = std::mem::take(&mut self.buffer);
                    events.push(make_math_event(&math));
                    self.state = State::Normal;
                } else {
                    self.state = State::MathBlockClosing { count: count + 1 };
                }
            } else {
                // Not closing ```, put backticks back into buffer
                for _ in 0..count {
                    self.buffer.push('`');
                }
                self.buffer.push(ch);
                self.state = State::MathBlock;
            }
        }
    }

    fn handle_bracket_math(&mut self, ch: char, events: &mut Vec<Event>) {
        if ch == ']' && self.buffer.ends_with('\\') {
            // Got \] closing
            self.buffer.pop(); // remove the backslash
            let math = std::mem::take(&mut self.buffer);
            events.push(make_math_event(&math));
            self.state = State::Normal;
        } else {
            self.buffer.push(ch);
        }
    }

    fn handle_paren_math(&mut self, ch: char, events: &mut Vec<Event>) {
        if ch == ')' && self.buffer.ends_with('\\') {
            // Got \) closing
            self.buffer.pop(); // remove the backslash
            let math = std::mem::take(&mut self.buffer);
            events.push(make_math_event(&math));
            self.state = State::Normal;
        } else {
            self.buffer.push(ch);
        }
    }

    fn handle_backslash_pending(&mut self, ch: char, events: &mut Vec<Event>) {
        match ch {
            '[' if self.config.display => {
                // \[ — display math
                self.buffer.clear();
                self.state = State::BracketMath;
            }
            '(' if self.config.display => {
                // \( — inline math (enabled with display since it has no shell collision risk)
                self.buffer.clear();
                self.state = State::ParenMath;
            }
            _ => {
                // Just a backslash followed by something else
                self.buffer.push(ch);
                events.push(Event::Text(std::mem::take(&mut self.buffer)));
                self.state = State::Normal;
            }
        }
    }

    fn handle_backtick1(&mut self, ch: char, events: &mut Vec<Event>) {
        if ch == '`' {
            self.buffer.push('`');
            self.state = State::Backtick2;
        } else {
            // Single backtick, not a math block
            self.buffer.push(ch);
            events.push(Event::Text(std::mem::take(&mut self.buffer)));
            self.state = State::Normal;
        }
    }

    fn handle_backtick2(&mut self, ch: char, events: &mut Vec<Event>) {
        if ch == '`' {
            self.buffer.push('`');
            self.state = State::BacktickTriple;
        } else {
            // Two backticks, not a math block
            self.buffer.push(ch);
            events.push(Event::Text(std::mem::take(&mut self.buffer)));
            self.state = State::Normal;
        }
    }

    fn handle_backtick_triple(&mut self, ch: char, events: &mut Vec<Event>) {
        // We've seen ```, now check for "math"
        self.buffer.push(ch);
        let after_ticks = &self.buffer[3..]; // content after ```
        if after_ticks.len() < 4 {
            // Still accumulating — check if it could be "math"
            if !"math"[..after_ticks.len()].starts_with(after_ticks) {
                // Doesn't match "math", emit as text
                events.push(Event::Text(std::mem::take(&mut self.buffer)));
                self.state = State::Normal;
            }
        } else if after_ticks == "math" {
            // Need newline or more content to confirm
            // Wait for next char
        } else if after_ticks.starts_with("math\n") || after_ticks.starts_with("math\r") {
            // Confirmed ```math block
            self.buffer.clear();
            self.state = State::MathBlock;
        } else {
            // Doesn't match
            events.push(Event::Text(std::mem::take(&mut self.buffer)));
            self.state = State::Normal;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::DelimiterConfig;

    fn all_config() -> DelimiterConfig {
        DelimiterConfig {
            block: true,
            display: true,
            inline: true,
        }
    }

    fn default_config() -> DelimiterConfig {
        DelimiterConfig {
            block: true,
            display: true,
            inline: false,
        }
    }

    #[test]
    fn test_plain_text() {
        let mut scanner = StreamScanner::new(default_config());
        let events = scanner.feed("hello world");
        let events = [events, scanner.flush()].concat();
        assert_eq!(events, vec![Event::Text("hello world".to_string())]);
    }

    #[test]
    fn test_dollar_dollar_math() {
        let mut scanner = StreamScanner::new(default_config());
        let events = scanner.feed("before $$\\frac{a}{b}$$ after");
        let mut all = events;
        all.extend(scanner.flush());
        assert_eq!(
            all,
            vec![
                Event::Text("before ".to_string()),
                Event::Math("\\frac{a}{b}".to_string()),
                Event::Text(" after".to_string()),
            ]
        );
    }

    #[test]
    fn test_math_block() {
        let mut scanner = StreamScanner::new(default_config());
        let events = scanner.feed("before\n```math\n\\frac{1}{2}\n```\nafter");
        let mut all = events;
        all.extend(scanner.flush());
        assert_eq!(
            all,
            vec![
                Event::Text("before\n".to_string()),
                Event::Math("\\frac{1}{2}".to_string()),
                Event::Text("\nafter".to_string()),
            ]
        );
    }

    #[test]
    fn test_ansi_passthrough() {
        let mut scanner = StreamScanner::new(default_config());
        let input = "hello \x1b[31mred\x1b[0m world";
        let events = scanner.feed(input);
        let mut all = events;
        all.extend(scanner.flush());
        // All events should be Text (ANSI passes through)
        let combined: String = all
            .iter()
            .map(|e| match e {
                Event::Text(t) => t.as_str(),
                Event::Math(_) => panic!("unexpected math"),
            })
            .collect();
        assert_eq!(combined, input);
    }

    #[test]
    fn test_bracket_math() {
        let mut scanner = StreamScanner::new(default_config());
        let events = scanner.feed("see \\[x^2\\] here");
        let mut all = events;
        all.extend(scanner.flush());
        assert_eq!(
            all,
            vec![
                Event::Text("see ".to_string()),
                Event::Math("x^2".to_string()),
                Event::Text(" here".to_string()),
            ]
        );
    }

    #[test]
    fn test_inline_dollar_with_backslash() {
        let mut scanner = StreamScanner::new(all_config());
        let events = scanner.feed("val $\\alpha$ end");
        let mut all = events;
        all.extend(scanner.flush());
        assert_eq!(
            all,
            vec![
                Event::Text("val ".to_string()),
                Event::Math("\\alpha".to_string()),
                Event::Text(" end".to_string()),
            ]
        );
    }

    #[test]
    fn test_shell_variable_skipped() {
        let mut scanner = StreamScanner::new(all_config());
        let events = scanner.feed("path is $HOME/bin");
        let mut all = events;
        all.extend(scanner.flush());
        // $HOME should not trigger math — no closing $ with \ inside
        let combined: String = all
            .iter()
            .map(|e| match e {
                Event::Text(t) => t.clone(),
                Event::Math(m) => format!("[MATH:{}]", m),
            })
            .collect();
        assert!(
            !combined.contains("[MATH:"),
            "should not detect shell var as math"
        );
    }

    #[test]
    fn test_streaming_chunks() {
        let mut scanner = StreamScanner::new(default_config());
        let mut all = Vec::new();
        all.extend(scanner.feed("before $$\\frac"));
        all.extend(scanner.feed("{a}{b}$$ after"));
        all.extend(scanner.flush());
        assert_eq!(
            all,
            vec![
                Event::Text("before ".to_string()),
                Event::Math("\\frac{a}{b}".to_string()),
                Event::Text(" after".to_string()),
            ]
        );
    }

    #[test]
    fn test_paren_math() {
        // \(...\) is gated by `display` (not `inline`) since it has no shell collision risk
        let mut scanner = StreamScanner::new(default_config());
        let events = scanner.feed("inline \\(x^2\\) here");
        let mut all = events;
        all.extend(scanner.flush());
        assert_eq!(
            all,
            vec![
                Event::Text("inline ".to_string()),
                Event::Math("x^2".to_string()),
                Event::Text(" here".to_string()),
            ]
        );
    }

    #[test]
    fn test_ansi_stripped_from_math() {
        let mut scanner = StreamScanner::new(default_config());
        // Math content with ANSI color codes mixed in
        let input = "$$\x1b[31m\\frac{a}{b}\x1b[0m$$";
        let events = scanner.feed(input);
        let mut all = events;
        all.extend(scanner.flush());
        assert_eq!(all, vec![Event::Math("\\frac{a}{b}".to_string())]);
    }

    #[test]
    fn test_strip_ansi_function() {
        assert_eq!(strip_ansi("hello"), "hello");
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;32mbold green\x1b[0m"), "bold green");
        assert_eq!(strip_ansi("no\x1b[Kansi"), "noansi");
        assert_eq!(strip_ansi("\x1b]0;title\x07text"), "text");
    }
}
