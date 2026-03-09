use crate::scanner::DelimiterConfig;
use crate::stream_scanner::StreamScanner;
use crate::RenderConfig;
use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Run a command inside a pty, intercepting output to render LaTeX.
pub fn run(
    command: &[String],
    config: &DelimiterConfig,
    render_config: &RenderConfig,
) -> Result<i32> {
    let pty_system = NativePtySystem::default();
    let (cols, rows) = terminal_size();

    crate::verbose_log(&format!(
        "wrapper: spawning {:?} in {}x{} pty",
        command, cols, rows
    ));

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open pty")?;

    let mut cmd = CommandBuilder::new(&command[0]);
    for arg in &command[1..] {
        cmd.arg(arg);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn command")?;

    let child_pid = child.process_id();
    crate::verbose_log(&format!("wrapper: child pid = {:?}", child_pid));

    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone pty reader")?;
    let mut writer = pair
        .master
        .take_writer()
        .context("Failed to get pty writer")?;

    // Signal handlers
    let winch_flag = Arc::new(AtomicBool::new(false));
    let term_flag = Arc::new(AtomicBool::new(false));
    let int_flag = Arc::new(AtomicBool::new(false));
    let hup_flag = Arc::new(AtomicBool::new(false));

    signal_hook::flag::register(libc::SIGWINCH, winch_flag.clone())
        .context("Failed to register SIGWINCH handler")?;
    signal_hook::flag::register(libc::SIGTERM, term_flag.clone())
        .context("Failed to register SIGTERM handler")?;
    signal_hook::flag::register(libc::SIGINT, int_flag.clone())
        .context("Failed to register SIGINT handler")?;
    signal_hook::flag::register(libc::SIGHUP, hup_flag.clone())
        .context("Failed to register SIGHUP handler")?;

    // Raw mode for stdin
    let orig_termios = set_raw_mode(libc::STDIN_FILENO);

    // Forward stdin → child pty
    thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Read child output via a channel so we can use recv_timeout for scanner timeout checks.
    // The blocking read happens in a separate thread; the main thread wakes every 50ms.
    let (tx, rx) = mpsc::sync_channel::<Option<Vec<u8>>>(16);
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(n) => {
                    if tx.send(Some(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });

    let mut scanner = StreamScanner::new(config.clone());
    let mut stdout = std::io::stdout();
    let render_config = render_config.clone();

    loop {
        // Handle SIGWINCH — resize child pty
        if winch_flag.swap(false, Ordering::Relaxed) {
            let (new_cols, new_rows) = terminal_size();
            crate::verbose_log(&format!(
                "wrapper: SIGWINCH, resizing to {}x{}",
                new_cols, new_rows
            ));
            let _ = pair.master.resize(PtySize {
                rows: new_rows,
                cols: new_cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }

        // Handle SIGINT/SIGTERM/SIGHUP — forward to child
        if int_flag.swap(false, Ordering::Relaxed) {
            forward_signal(child_pid, libc::SIGINT);
        }
        if term_flag.swap(false, Ordering::Relaxed) {
            forward_signal(child_pid, libc::SIGTERM);
        }
        if hup_flag.swap(false, Ordering::Relaxed) {
            forward_signal(child_pid, libc::SIGHUP);
        }

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Some(data)) => {
                let chunk = String::from_utf8_lossy(&data);
                let events = scanner.feed(&chunk);
                crate::process_events(&events, &mut stdout, &render_config);

                let timeout_events = scanner.check_timeout();
                crate::process_events(&timeout_events, &mut stdout, &render_config);
            }
            Ok(None) => break, // EOF
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Check scanner timeouts (e.g. $ delimiter timeout)
                let timeout_events = scanner.check_timeout();
                crate::process_events(&timeout_events, &mut stdout, &render_config);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let final_events = scanner.flush();
    crate::process_events(&final_events, &mut stdout, &render_config);

    if let Some(orig) = orig_termios {
        restore_termios(libc::STDIN_FILENO, &orig);
    }

    let status = child.wait().context("Failed to wait for child process")?;
    let code: i32 = status.exit_code().try_into().unwrap_or(1);
    crate::verbose_log(&format!("wrapper: child exited with code {}", code));
    Ok(code)
}

fn forward_signal(pid: Option<u32>, sig: i32) {
    if let Some(pid) = pid {
        crate::verbose_log(&format!(
            "wrapper: forwarding signal {} to pid {}",
            sig, pid
        ));
        unsafe {
            libc::kill(pid as i32, sig);
        }
    }
}

fn terminal_size() -> (u16, u16) {
    term_size().unwrap_or((80, 24))
}

fn term_size() -> Option<(u16, u16)> {
    unsafe {
        let mut ws = std::mem::zeroed::<libc::winsize>();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 {
            Some((ws.ws_col, ws.ws_row))
        } else {
            None
        }
    }
}

fn set_raw_mode(fd: i32) -> Option<libc::termios> {
    unsafe {
        let mut orig = std::mem::zeroed::<libc::termios>();
        if libc::tcgetattr(fd, &mut orig) != 0 {
            return None;
        }
        let mut raw = orig;
        libc::cfmakeraw(&mut raw);
        if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
            return None;
        }
        Some(orig)
    }
}

fn restore_termios(fd: i32, orig: &libc::termios) {
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, orig);
    }
}
