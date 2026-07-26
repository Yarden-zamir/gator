//! Shared screen layout and help-footer rendering for the gator app family.
//!
//! Every app uses the same shell: a list pane with a one-line search field on
//! the left, a detail/preview pane on the right, and a keys footer underneath.

use crate::keymap::{format_chord_label, BindingContext, BindingTarget, CoreAction, Keymap};
use crate::theme::Palette;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

/// Regions of the standard two-pane screen.
pub struct SplitLayout {
    /// The whole left pane, including its border.
    pub left: Rect,
    /// One-line search input inside the left pane.
    pub search: Rect,
    /// Scrollable list below the search line.
    pub results: Rect,
    /// The right-hand detail/preview pane.
    pub detail: Rect,
    /// The keys footer.
    pub help: Rect,
}

/// Build the standard layout, giving the left pane `left_percent` of the width.
pub fn split_layout(size: Rect, left_percent: u16) -> SplitLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(size);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_percent),
            Constraint::Percentage(100u16.saturating_sub(left_percent)),
        ])
        .split(chunks[0]);
    let left_inner = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .inner(body[0]);
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(left_inner);
    SplitLayout {
        left: body[0],
        search: left_chunks[0],
        results: left_chunks[1],
        detail: body[1],
        help: chunks[1],
    }
}

/// One `key label` pair in the help footer.
pub struct HelpHint {
    pub key: String,
    pub label: String,
    /// Render the label in the accent color (used for live state such as the
    /// current sort or filter mode).
    pub accent: bool,
}

impl HelpHint {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            accent: false,
        }
    }

    /// A hint whose label shows current state and is accented.
    pub fn state(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            accent: true,
        }
    }
}

/// Render help hints as a single footer line.
pub fn help_line(hints: &[HelpHint], palette: &Palette) -> Line<'static> {
    let key_style = Style::default()
        .fg(palette.key)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(palette.text);
    let accent_style = Style::default().fg(palette.accent);
    let mut spans = Vec::with_capacity(hints.len() * 2);
    for hint in hints {
        spans.push(Span::styled(hint.key.clone(), key_style));
        let label_style = if hint.accent {
            accent_style
        } else {
            text_style
        };
        spans.push(Span::styled(format!(" {}  ", hint.label), label_style));
    }
    Line::from(spans)
}

/// A help hint for `action`, labeled with the chord currently bound to it in
/// `context`. Returns `None` when the action is unbound, so help never
/// advertises a key that does nothing.
pub fn binding_hint<C: BindingContext, A: CoreAction>(
    keymap: &Keymap<C, A>,
    context: C,
    action: A,
    label: impl Into<String>,
) -> Option<HelpHint> {
    keymap
        .first_chord_for_target(context, &BindingTarget::Core(action))
        .map(|chord| HelpHint::new(format_chord_label(chord), label))
}

/// Like [`binding_hint`], but the label is accented to show live state.
pub fn binding_state_hint<C: BindingContext, A: CoreAction>(
    keymap: &Keymap<C, A>,
    context: C,
    action: A,
    label: impl Into<String>,
) -> Option<HelpHint> {
    keymap
        .first_chord_for_target(context, &BindingTarget::Core(action))
        .map(|chord| HelpHint::state(format_chord_label(chord), label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_layout_divides_width_by_percentage() {
        let area = Rect::new(0, 0, 100, 40);
        let layout = split_layout(area, 55);
        assert_eq!(layout.left.width, 55);
        assert_eq!(layout.detail.width, 45);
        assert_eq!(layout.help.height, 3);
        // search sits directly above the results list, inside the left border
        assert_eq!(layout.search.height, 1);
        assert_eq!(layout.results.y, layout.search.y + 1);
    }

    #[test]
    fn help_line_pairs_keys_with_labels() {
        let palette = Palette::light();
        let line = help_line(
            &[
                HelpHint::new("Enter", "open"),
                HelpHint::state("^s", "sort"),
            ],
            &palette,
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|span| span.content.to_string())
            .collect();
        assert_eq!(rendered, "Enter open  ^s sort  ");
    }
}
