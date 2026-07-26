//! Text, layout, and color helpers shared across the gator app family:
//! truncation, home-path display, ANSI/plain line building, rect math, and
//! color conversion.

use ansi_to_tui::IntoText;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
};

/// Truncate `value` to at most `max` characters, replacing the overflow with a
/// single `…`. Returns an empty string when `max` is 0.
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

/// Collapse a leading `home` prefix in `path` to `~`. An empty `home`, or a
/// `path` outside `home`, is returned unchanged.
pub fn collapse_home(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_string();
    }
    if path == home {
        return "~".to_string();
    }
    let home_with_separator = format!(
        "{}{}",
        home.trim_end_matches(std::path::MAIN_SEPARATOR),
        std::path::MAIN_SEPARATOR
    );
    if let Some(rest) = path.strip_prefix(&home_with_separator) {
        return format!("~/{rest}");
    }
    path.to_string()
}

/// [`collapse_home`] using the `HOME` environment variable; returns `path`
/// unchanged when `HOME` is unset.
pub fn collapse_home_env(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) => collapse_home(path, &home),
        Err(_) => path.to_string(),
    }
}

/// Build up to `max_lines` styled lines from plain text, one per input line.
pub fn lines_from_output(output: &str, style: Style, max_lines: usize) -> Vec<Line<'static>> {
    output
        .lines()
        .take(max_lines)
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

/// Build up to `max_lines` lines from text that may contain ANSI escapes,
/// falling back to [`lines_from_output`] when the input does not parse.
pub fn lines_from_ansi_output(output: &str, style: Style, max_lines: usize) -> Vec<Line<'static>> {
    let Ok(text) = output.as_bytes().to_vec().into_text() else {
        return lines_from_output(output, style, max_lines);
    };
    text.lines
        .into_iter()
        .take(max_lines)
        .map(|line| line.style(style))
        .collect()
}

/// Convert ANSI text to a ratatui [`Text`], falling back to a single plain
/// block styled with `style` when parsing fails.
pub fn text_from_ansi(output: &str, style: Style) -> Text<'static> {
    match output.as_bytes().to_vec().into_text() {
        Ok(text) => text,
        Err(_) => Text::from(output.to_string()).patch_style(style),
    }
}

/// Whether the cell at `(col, row)` lies inside `rect`.
pub fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// A `width`×`height` rect centered within `area` (clamped so it never starts
/// before `area`).
pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

/// Render a `[████░░░░]` progress bar for `progress` in `0.0..=1.0`. The bar
/// width derives from `width` (clamped to `8..=72`); `fill`/`empty` color the
/// filled and remaining cells.
pub fn render_progress_bar(
    progress: f64,
    width: usize,
    fill: Color,
    empty: Color,
) -> Line<'static> {
    let bar_width = width.saturating_sub(6).clamp(8, 72);
    let filled = ((bar_width as f64) * progress).round() as usize;
    let empty_count = bar_width.saturating_sub(filled);
    Line::from(vec![
        Span::styled("[", Style::default().fg(empty)),
        Span::styled("█".repeat(filled), Style::default().fg(fill)),
        Span::styled("░".repeat(empty_count), Style::default().fg(empty)),
        Span::styled("]", Style::default().fg(empty)),
    ])
}

/// Convert HSL (`hue` in `0..=360`, `sat`/`light` in `0.0..=1.0`) to an RGB
/// [`Color`], or `None` when `hue` is out of range.
pub fn hsl_to_rgb(hue: f32, sat: f32, light: f32) -> Option<Color> {
    if !(0.0..=360.0).contains(&hue) {
        return None;
    }
    let c = (1.0 - (2.0 * light - 1.0).abs()) * sat;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if (0.0..1.0).contains(&h) {
        (c, x, 0.0)
    } else if (1.0..2.0).contains(&h) {
        (x, c, 0.0)
    } else if (2.0..3.0).contains(&h) {
        (0.0, c, x)
    } else if (3.0..4.0).contains(&h) {
        (0.0, x, c)
    } else if (4.0..5.0).contains(&h) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = light - c / 2.0;
    let r = ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_with_single_ellipsis() {
        assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
        assert_eq!(truncate_with_ellipsis("hello", 3), "he…");
        assert_eq!(truncate_with_ellipsis("hello", 0), "");
    }

    #[test]
    fn collapses_home_prefix() {
        assert_eq!(collapse_home("/home/x/p", "/home/x"), "~/p");
        assert_eq!(collapse_home("/home/x", "/home/x"), "~");
        assert_eq!(collapse_home("/other", "/home/x"), "/other");
        assert_eq!(collapse_home("/home/x/p", ""), "/home/x/p");
    }

    #[test]
    fn rect_contains_respects_bounds() {
        let rect = Rect::new(2, 3, 4, 5);
        assert!(rect_contains(rect, 2, 3));
        assert!(rect_contains(rect, 5, 7));
        assert!(!rect_contains(rect, 6, 3));
        assert!(!rect_contains(rect, 2, 8));
    }
}
