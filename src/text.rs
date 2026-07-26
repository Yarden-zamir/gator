//! Text, layout, and color helpers shared across the gator app family:
//! truncation, home-path display, ANSI/plain line building, rect math, and
//! color conversion.

use ansi_to_tui::IntoText;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use tui_input::{Input, InputRequest};

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

/// Number of rendered lines in `text`.
pub fn text_line_count(text: &Text) -> usize {
    text.lines.len()
}

/// Insert pasted `value` into `input`, dropping carriage returns so bracketed
/// paste of multi-line text does not split the field.
pub fn insert_paste(input: &mut Input, value: &str) {
    for ch in value.chars().filter(|ch| *ch != '\r') {
        input.handle(InputRequest::InsertChar(ch));
    }
}

/// Scroll offset that keeps `selected` visible in a `height`-row window over
/// `total` items, moving as little as possible.
pub fn list_window_offset(selected: usize, offset: usize, height: usize, total: usize) -> usize {
    if height == 0 || total == 0 {
        return 0;
    }
    let mut offset = offset.min(total.saturating_sub(1));
    if selected < offset {
        offset = selected;
    } else if selected >= offset + height {
        offset = selected + 1 - height;
    }
    offset.min(total.saturating_sub(height.min(total)))
}

/// Hard-wrap `line` into chunks of at most `width` characters. An empty line
/// yields one empty chunk so blank lines survive wrapping.
pub fn wrap_text_line(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if line.is_empty() {
        return vec![String::new()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        if current.chars().count() >= width {
            chunks.push(current);
            current = String::new();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Split `line` into spans, rendering case-insensitive `needle` matches in
/// reverse video. Falls back to a single unhighlighted span when the needle is
/// empty or when lowercasing shifts byte offsets (multi-byte text).
pub fn highlight_line(line: &str, needle: Option<&str>, text_color: Color) -> Line<'static> {
    let base = Style::default().fg(text_color);
    let Some(needle) = needle.map(str::trim).filter(|needle| !needle.is_empty()) else {
        return Line::from(Span::styled(line.to_string(), base));
    };
    let lower_line = line.to_lowercase();
    let lower_needle = needle.to_lowercase();
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    while let Some(found) = lower_line[cursor..].find(&lower_needle) {
        let start = cursor + found;
        let end = start + lower_needle.len();
        if !line.is_char_boundary(start) || !line.is_char_boundary(end) || end > line.len() {
            return Line::from(Span::styled(line.to_string(), base));
        }
        if start > cursor {
            spans.push(Span::styled(line[cursor..start].to_string(), base));
        }
        spans.push(Span::styled(
            line[start..end].to_string(),
            base.add_modifier(Modifier::REVERSED),
        ));
        cursor = end;
    }
    if cursor < line.len() {
        spans.push(Span::styled(line[cursor..].to_string(), base));
    }
    if spans.is_empty() {
        return Line::from(Span::styled(line.to_string(), base));
    }
    Line::from(spans)
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
    fn window_offset_follows_selection() {
        assert_eq!(list_window_offset(0, 0, 10, 100), 0);
        assert_eq!(list_window_offset(15, 0, 10, 100), 6);
        assert_eq!(list_window_offset(3, 6, 10, 100), 3);
        assert_eq!(list_window_offset(0, 0, 0, 0), 0);
        // window never scrolls past the end when items fit
        assert_eq!(list_window_offset(2, 0, 10, 3), 0);
    }

    #[test]
    fn highlight_marks_every_match() {
        let line = highlight_line("the Rate limiter rate", Some("rate"), Color::Black);
        let reversed: Vec<String> = line
            .spans
            .iter()
            .filter(|span| span.style.add_modifier.contains(Modifier::REVERSED))
            .map(|span| span.content.to_string())
            .collect();
        assert_eq!(reversed, vec!["Rate", "rate"]);
    }

    #[test]
    fn highlight_survives_multibyte_without_panicking() {
        let _ = highlight_line("préfix — ünïcode", Some("é"), Color::Black);
        let _ = highlight_line("emoji 🎉 test", Some("test"), Color::Black);
    }

    #[test]
    fn wraps_lines_and_keeps_blank_lines() {
        assert_eq!(wrap_text_line("abcdef", 3), vec!["abc", "def"]);
        assert_eq!(wrap_text_line("", 5), vec![String::new()]);
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
