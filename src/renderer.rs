use base64::Engine;
use std::io::{self, Write};

/// Render Unicode art to stdout.
pub fn render_unicode(content: &str) {
    println!("{}", content);
}

/// Get the terminal width in columns.
fn terminal_cols() -> u16 {
    unsafe {
        let mut ws = std::mem::zeroed::<libc::winsize>();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 {
            ws.ws_col
        } else {
            80
        }
    }
}

/// Read PNG width from the IHDR chunk header.
fn png_width(data: &[u8]) -> Option<u32> {
    // PNG: 8-byte signature + IHDR chunk (length[4] + "IHDR"[4] + width[4])
    if data.len() < 24 {
        return None;
    }
    let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    Some(w)
}

/// Compute display columns for an image, capping at terminal width.
/// Kitty/iTerm images are rendered at ~8px per cell (typical).
fn image_display_cols(png_data: &[u8]) -> Option<u32> {
    let img_width = png_width(png_data)?;
    let term_cols = terminal_cols() as u32;
    // At 144 PPI rendering, each character cell is ~8px wide
    let cell_px = 8u32;
    let img_cols = img_width.div_ceil(cell_px);
    if img_cols > term_cols {
        Some(term_cols)
    } else {
        None // no limiting needed
    }
}

/// Render a PNG image using the Kitty Graphics Protocol.
/// Format: ESC _G key=value,...; <base64 data> ESC \
pub fn render_kitty_image(png_data: &[u8]) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
    let mut stdout = io::stdout();
    let chunk_size = 4096;
    let bytes = encoded.as_bytes();

    // Build column constraint if image is wider than terminal
    let col_spec = match image_display_cols(png_data) {
        Some(cols) => format!(",c={}", cols),
        None => String::new(),
    };

    if bytes.len() <= chunk_size {
        let _ = write!(stdout, "\x1b_Gf=100,a=T{};{}\x1b\\", col_spec, encoded);
    } else {
        let chunks: Vec<&[u8]> = bytes.chunks(chunk_size).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let data = std::str::from_utf8(chunk).unwrap_or("");
            let more = if i < chunks.len() - 1 { 1 } else { 0 };
            if i == 0 {
                let _ = write!(
                    stdout,
                    "\x1b_Gf=100,a=T,m={}{};{}\x1b\\",
                    more, col_spec, data
                );
            } else {
                let _ = write!(stdout, "\x1b_Gm={};{}\x1b\\", more, data);
            }
        }
    }
    let _ = writeln!(stdout);
    let _ = stdout.flush();
}

/// Render a PNG image using the iTerm2 inline image protocol.
/// Format: ESC ] 1337;File=inline=1;size=<n>:<base64> BEL
pub fn render_iterm2_image(png_data: &[u8]) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
    let size = png_data.len();
    let mut stdout = io::stdout();

    // Limit width to terminal columns if image is too wide
    let width_spec = match image_display_cols(png_data) {
        Some(cols) => format!(";width={}", cols),
        None => String::new(),
    };

    let _ = write!(
        stdout,
        "\x1b]1337;File=inline=1;size={};preserveAspectRatio=1{}:{}\x07",
        size, width_spec, encoded
    );
    let _ = writeln!(stdout);
    let _ = stdout.flush();
}
