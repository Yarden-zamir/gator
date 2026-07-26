//! Shared color themes for the gator app family.
//!
//! Every gator app uses the same accent/warm/key/text/muted palette and the
//! same `auto`/`light`/`dark` selection, including OS dark-mode detection.

use ratatui::style::Color;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Theme selection as it appears in a `[ui] theme = "..."` config value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    /// Follow the operating system appearance.
    #[default]
    Auto,
    Light,
    Dark,
}

impl Theme {
    /// Parse a theme name, returning a user-facing error for unknown values.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(format!(
                "invalid theme {value:?}; expected auto, light, or dark"
            )),
        }
    }
}

/// The five color roles every gator app renders with.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub accent: Color,
    pub warm: Color,
    pub key: Color,
    pub text: Color,
    pub muted: Color,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            accent: Color::Rgb(72, 166, 255),
            warm: Color::Rgb(255, 181, 92),
            key: Color::Rgb(150, 150, 150),
            text: Color::Black,
            muted: Color::Black,
        }
    }

    pub fn dark() -> Self {
        Self {
            accent: Color::Rgb(99, 179, 237),
            warm: Color::Rgb(251, 191, 36),
            key: Color::Rgb(156, 163, 175),
            text: Color::Rgb(229, 231, 235),
            muted: Color::Rgb(156, 163, 175),
        }
    }

    /// Resolve a palette for a theme, consulting the OS appearance for `Auto`.
    pub fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Auto if os_prefers_dark_theme() => Self::dark(),
            Theme::Auto | Theme::Light => Self::light(),
            Theme::Dark => Self::dark(),
        }
    }
}

/// Whether the operating system currently prefers a dark appearance.
///
/// Only macOS is probed today; other platforms report `false`.
pub fn os_prefers_dark_theme() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("defaults")
            .arg("read")
            .arg("-g")
            .arg("AppleInterfaceStyle")
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .eq_ignore_ascii_case("Dark")
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_names() {
        assert_eq!(Theme::parse("auto"), Ok(Theme::Auto));
        assert_eq!(Theme::parse("light"), Ok(Theme::Light));
        assert_eq!(Theme::parse("dark"), Ok(Theme::Dark));
        assert!(Theme::parse("technicolor").is_err());
    }

    #[test]
    fn explicit_themes_do_not_consult_the_os() {
        // Light/Dark must be deterministic regardless of OS appearance.
        let light = Palette::for_theme(Theme::Light);
        let dark = Palette::for_theme(Theme::Dark);
        assert_eq!(light.accent, Color::Rgb(72, 166, 255));
        assert_eq!(dark.accent, Color::Rgb(99, 179, 237));
    }
}
