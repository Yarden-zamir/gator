use std::{env, error::Error, fs, io, path::Path, process::Command};

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_input::Input;

pub type AppResult<T> = Result<T, Box<dyn Error>>;

pub type AppTerminal = Terminal<CrosstermBackend<io::Stderr>>;

pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stderr(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        );
    }
}

pub fn ensure_tty_stdin() -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::io::IsTerminal;
        use std::os::unix::io::AsRawFd;

        if io::stdin().is_terminal() {
            return Ok(());
        }

        let tty = fs::File::open("/dev/tty")?;
        let result = unsafe { libc::dup2(tty.as_raw_fd(), libc::STDIN_FILENO) };
        if result == -1 {
            return Err(io::Error::last_os_error().into());
        }
    }
    Ok(())
}

pub fn setup_terminal() -> AppResult<(AppTerminal, TerminalGuard)> {
    enable_raw_mode()?;
    execute!(
        io::stderr(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    Ok((terminal, TerminalGuard))
}

pub fn write_selection(value: &str) -> AppResult<()> {
    if let Ok(output_path) = env::var("GATOR_OUTPUT") {
        if !output_path.is_empty() {
            fs::write(output_path, value)?;
            return Ok(());
        }
    }
    println!("{value}");
    Ok(())
}

pub fn input_at_end(input: &Input) -> bool {
    input.cursor() >= input.value().chars().count()
}

pub fn truncate_with_ellipsis(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = value.chars().count();
    if count <= max {
        return value.to_string();
    }
    if max <= 1 {
        return value.chars().take(max).collect();
    }
    let trimmed = value.chars().take(max - 1).collect::<String>();
    format!("{trimmed}…")
}

pub fn run_command_output(
    program: &str,
    args: &[String],
    current_dir: Option<&Path>,
) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

pub fn copy_to_clipboard(value: &str) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        use std::{io::Write, process::Stdio};

        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
        let Some(stdin) = child.stdin.as_mut() else {
            return Err("failed to open pbcopy stdin".into());
        };
        stdin.write_all(value.as_bytes())?;
        let status = child.wait()?;
        if !status.success() {
            return Err("pbcopy failed".into());
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = value;
        Err("clipboard copy is only implemented for macOS".into())
    }
}

pub fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut query_chars = query.chars().filter(|c| !c.is_whitespace());
    let mut current = query_chars.next();
    if current.is_none() {
        return true;
    }
    for ch in text.chars() {
        if let Some(expected) = current {
            if expected.eq_ignore_ascii_case(&ch) {
                current = query_chars.next();
                if current.is_none() {
                    return true;
                }
            }
        }
    }
    false
}
