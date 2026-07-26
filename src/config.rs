//! Generic TOML configuration engine shared across the gator app family.
//!
//! Provides file discovery, layered loading (files then CLI overrides), JSON
//! Schema generation, `$schema` link injection, default-file scaffolding, and
//! path normalization. Each app supplies its own config struct (deriving
//! `Deserialize` + `JsonSchema` and implementing [`AppConfig`]) and a merge
//! function; the discovery, layering, and schema plumbing live here and behave
//! identically for every app.

use crate::text::collapse_home_env;
use crate::AppResult;
use figment::providers::{Format, Toml};
use figment::Figment;
use schemars::JsonSchema;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Which layer a parsed config came from. File layers merge in discovery order;
/// CLI layers apply last and win.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerSource {
    File,
    Cli,
}

/// A config file type that can report whether it already declared a `$schema`
/// value, so the loader knows when to inject one.
pub trait AppConfig: serde::de::DeserializeOwned {
    fn has_schema_url(&self) -> bool;
}

/// `$HOME` as a path, or an error when it is unset.
pub fn home_dir() -> AppResult<PathBuf> {
    let value = env::var("HOME").map_err(|_| "HOME is not set")?;
    Ok(PathBuf::from(value))
}

/// XDG config root (`$XDG_CONFIG_HOME` or `~/.config`).
pub fn config_home(home: &Path) -> PathBuf {
    env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
}

/// A non-empty, trimmed environment variable interpreted as a path.
pub fn env_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// TOML-quote and escape a string value.
pub fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Expand `~/` and `$HOME`, then resolve relative paths against `base_dir`.
/// Does not require the path to exist.
pub fn normalize_configured_path(raw: &str, base_dir: &Path, home: &Path) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut value = trimmed.to_string();
    if value.starts_with("~/") {
        value = value.replacen('~', &home.to_string_lossy(), 1);
    }
    if value.contains("$HOME") {
        value = value.replace("$HOME", &home.to_string_lossy());
    }
    let mut path = PathBuf::from(value);
    if path.is_relative() {
        path = base_dir.join(path);
    }
    Some(path)
}

/// Like [`normalize_configured_path`] but keeps only paths that exist.
pub fn normalize_existing_path(raw: &str, base_dir: &Path, home: &Path) -> Option<PathBuf> {
    let path = normalize_configured_path(raw, base_dir, home)?;
    path.exists().then_some(path)
}

/// Normalize and append each existing path in `raw_paths` to `target`,
/// deduplicating by normalized string via `seen`.
pub fn merge_paths(
    raw_paths: &[String],
    base_dir: &Path,
    home: &Path,
    target: &mut Vec<PathBuf>,
    seen: &mut HashSet<String>,
) {
    for raw in raw_paths {
        if let Some(path) = normalize_existing_path(raw, base_dir, home) {
            let key = path.to_string_lossy().to_string();
            if seen.insert(key) {
                target.push(path);
            }
        }
    }
}

/// Generate the pretty-printed JSON Schema for a config type.
pub fn schema_json<T: JsonSchema>() -> AppResult<String> {
    let schema = schemars::schema_for!(T);
    serde_json::to_string_pretty(&schema)
        .map_err(|err| format!("Failed to serialize config schema: {err}").into())
}

/// Prepend a `"$schema" = "<url>"` line to `path` when the parsed config did
/// not already declare one (`already_present == false`). Best-effort: I/O
/// errors are ignored so a read-only config never blocks startup.
pub fn ensure_schema_link(path: &Path, schema_url: &str, already_present: bool) {
    if already_present {
        return;
    }
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };
    let schema_line = format!("\"$schema\" = \"{schema_url}\"");
    let updated = if contents.trim().is_empty() {
        format!("{schema_line}\n")
    } else if contents.starts_with('\n') {
        format!("{schema_line}\n{contents}")
    } else {
        format!("{schema_line}\n\n{contents}")
    };
    if updated != contents {
        let _ = fs::write(path, updated);
    }
}

/// App metadata that parameterizes config discovery and scaffolding.
pub struct ConfigLoader<'a> {
    /// Directory/file stem, e.g. `"navgator"`.
    pub app_name: &'a str,
    /// Absolute-path override env var, e.g. `"NAVGATOR_CONFIG"`.
    pub env_var: &'a str,
    /// `$schema` URL written into new/unlinked config files.
    pub schema_url: &'a str,
    /// Contents written when no config file exists yet.
    pub default_contents: &'a str,
}

impl ConfigLoader<'_> {
    /// Config file search path, most general first: `$<ENV>` override (used
    /// alone if set), then `/etc/<app>`, XDG, `~/.config/<app>`,
    /// `~/.<app>.toml`, and cwd `.<app>.toml` / `.<app>/config.toml`.
    /// Deduplicated by string.
    pub fn config_paths(&self, home: &Path) -> Vec<PathBuf> {
        if let Some(path) = env_path(self.env_var) {
            return vec![path];
        }

        let app = self.app_name;
        let mut paths = Vec::new();
        paths.push(PathBuf::from(format!("/etc/{app}/config.toml")));
        let xdg = config_home(home);
        paths.push(xdg.join(format!("{app}/config.toml")));
        paths.push(home.join(format!(".config/{app}/config.toml")));
        paths.push(home.join(format!(".{app}.toml")));
        if let Ok(cwd) = env::current_dir() {
            paths.push(cwd.join(format!(".{app}.toml")));
            paths.push(cwd.join(format!(".{app}/config.toml")));
        }

        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        for path in paths {
            let key = path.to_string_lossy().to_string();
            if seen.insert(key) {
                unique.push(path);
            }
        }
        unique
    }

    /// Path a first-run default config is written to (respecting the env
    /// override, else XDG `~/.config/<app>/config.toml`).
    pub fn default_config_path(&self, home: &Path) -> PathBuf {
        if let Some(path) = env_path(self.env_var) {
            return path;
        }
        config_home(home).join(format!("{}/config.toml", self.app_name))
    }

    /// Discover and load config files into `state` via `merge`, apply each CLI
    /// `entry` as a higher-priority TOML layer, then `finalize`. When no config
    /// file exists, a default is written and reloaded once. Mirrors the shared
    /// navgator loading contract.
    pub fn load<T, S, M, F, R>(
        &self,
        cli_entries: &[String],
        mut state: S,
        mut merge: M,
        finalize: F,
    ) -> AppResult<R>
    where
        T: AppConfig,
        M: FnMut(&mut S, T, &Path, &Path, LayerSource) -> AppResult<()>,
        F: FnOnce(S) -> AppResult<R>,
    {
        let home = home_dir()?;

        let mut found = self.load_files(&home, &mut state, &mut merge)?;
        if !found {
            let default_path = self.default_config_path(&home);
            self.create_default(&default_path)?;
            found = self.load_files(&home, &mut state, &mut merge)?;
            let _ = found;
        }

        let base_dir = env::current_dir().unwrap_or_else(|_| home.clone());
        for (index, entry) in cli_entries.iter().enumerate() {
            let config: T = Figment::from(Toml::string(entry))
                .extract()
                .map_err(|err| {
                    format!(
                        "Failed to parse config entry {} ({entry:?}): {err}",
                        index + 1
                    )
                })?;
            merge(&mut state, config, &base_dir, &home, LayerSource::Cli).map_err(|err| {
                format!(
                    "Failed to apply config entry {} ({entry:?}): {err}",
                    index + 1
                )
            })?;
        }

        finalize(state)
    }

    fn load_files<T, S, M>(&self, home: &Path, state: &mut S, merge: &mut M) -> AppResult<bool>
    where
        T: AppConfig,
        M: FnMut(&mut S, T, &Path, &Path, LayerSource) -> AppResult<()>,
    {
        let mut found = false;
        for path in self.config_paths(home) {
            if !path.is_file() {
                continue;
            }
            found = true;
            let base_dir = path.parent().unwrap_or(home).to_path_buf();
            let config: T = Figment::from(Toml::file(&path)).extract().map_err(|err| {
                let display_path = collapse_home_env(&path.to_string_lossy());
                format!("Failed to parse config {display_path}: {err}")
            })?;
            ensure_schema_link(&path, self.schema_url, config.has_schema_url());
            merge(state, config, &base_dir, home, LayerSource::File)?;
        }
        Ok(found)
    }

    fn create_default(&self, path: &Path) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create config directory {}: {err}",
                    collapse_home_env(&parent.to_string_lossy())
                )
            })?;
        }
        fs::write(path, self.default_contents).map_err(|err| {
            format!(
                "Failed to create default config {}: {err}",
                collapse_home_env(&path.to_string_lossy())
            )
        })?;
        Ok(())
    }
}
