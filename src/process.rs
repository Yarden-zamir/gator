//! Subprocess helpers: capturing command output, running interactive or shell
//! commands, and opening URLs, shared across the gator app family.

use std::{path::Path, process::Command};

/// Run `program` with `args`, returning trimmed stdout on success, or `None`
/// when the command fails to spawn, exits non-zero, or produces empty output.
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

/// Run `program` inheriting the parent's stdio, mapping a non-zero exit or
/// spawn failure to a user-facing error string.
pub fn run_interactive_command(
    program: &str,
    args: &[String],
    current_dir: Option<&Path>,
) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    }
    let status = cmd
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

/// Captured result of a shell recipe.
pub struct ShellRecipeResult {
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

/// Run `shell` through the platform shell (`sh -c` / `cmd /C`) with extra env
/// vars and `NO_COLOR=1`, capturing output. Non-zero exits become an error that
/// includes stderr when present.
pub fn run_shell_recipe(
    shell: &str,
    current_dir: Option<&Path>,
    envs: &[(String, String)],
) -> Result<ShellRecipeResult, String> {
    let mut cmd = shell_command(shell);
    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    }
    for (name, value) in envs {
        cmd.env(name, value);
    }
    let output = cmd
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| format!("failed to run shell recipe: {err}"))?;
    let result = ShellRecipeResult {
        status: output.status.to_string(),
        stdout: String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_string(),
    };
    if output.status.success() {
        Ok(result)
    } else if result.stderr.is_empty() {
        Err(format!("shell recipe exited with {}", result.status))
    } else {
        Err(format!(
            "shell recipe exited with {}: {}",
            result.status, result.stderr
        ))
    }
}

#[cfg(target_os = "windows")]
fn shell_command(shell: &str) -> Command {
    let mut command = Command::new("cmd");
    command.args(["/C", shell]);
    command
}

#[cfg(not(target_os = "windows"))]
fn shell_command(shell: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(shell);
    command
}

/// Open `url` with the platform opener (`open` / `start` / `xdg-open`).
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let args = vec![url.to_string()];
        run_interactive_command("open", &args, None)
    }

    #[cfg(target_os = "windows")]
    {
        let args = vec!["/C".to_string(), "start".to_string(), url.to_string()];
        run_interactive_command("cmd", &args, None)
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let args = vec![url.to_string()];
        run_interactive_command("xdg-open", &args, None)
    }
}
