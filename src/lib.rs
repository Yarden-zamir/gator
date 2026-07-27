//! Shared TUI and tooling primitives for the gator app family.
//!
//! `gator` holds only generic infrastructure — terminal setup, selection
//! output, clipboard, subprocess helpers, fuzzy matching, theming, config
//! loading, keybindings, and small TUI helpers. Domain behavior (git, GitHub,
//! project navigation, issue exploration, session sources) lives in the
//! implementation crates.

use std::{env, error::Error, fs, io};

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_input::Input;

pub mod config;
pub mod keymap;
pub mod layout;
pub mod process;
pub mod search;
pub mod text;
pub mod theme;
pub mod xdg;

// Backward-compatible flat re-exports of the most-used helpers.
pub use process::run_command_output;
pub use search::fuzzy_match;
pub use text::truncate_with_ellipsis;

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

pub fn copy_to_clipboard(value: &str) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        // Imported here so the non-macOS build does not see them as unused.
        use std::{
            io::Write,
            process::{Command, Stdio},
        };

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
