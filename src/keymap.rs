//! Generic, config-driven keybinding engine shared across the gator app
//! family.
//!
//! Chord parsing/formatting (`"ctrl+enter"` ↔ [`KeyChord`]) and the resolution
//! engine ([`Keymap`]) are provided here. Each app supplies its own set of UI
//! contexts and semantic actions by implementing [`BindingContext`] and
//! [`CoreAction`]; the engine is generic over both, so apps keep their
//! exhaustive enums while the algorithms live here.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::Hash;

/// A UI context (screen/mode) that bindings are scoped to.
///
/// Implementors are concrete enums; the engine only needs string
/// serialization, the canonical ordering, and each context's fallback chain
/// (most-specific first, e.g. `Preview → Navigator → Global`).
pub trait BindingContext: Copy + Ord + Hash + fmt::Debug + 'static {
    fn as_str(self) -> &'static str;
    fn parse(value: &str) -> Option<Self>
    where
        Self: Sized;
    /// Canonical list of all contexts, in documentation order.
    fn ordered() -> &'static [Self]
    where
        Self: Sized;
    /// Contexts to consult when resolving an event in `self`, most specific
    /// first. Must start with `self`.
    fn fallback_contexts(self) -> &'static [Self]
    where
        Self: Sized;
}

/// A built-in semantic action an app understands.
///
/// Implementors are concrete enums providing string serialization; custom,
/// config-defined action ids are carried separately by
/// [`BindingTarget::Configured`].
pub trait CoreAction: Copy + Eq + Hash + fmt::Debug {
    fn as_str(self) -> &'static str;
    fn parse(value: &str) -> Option<Self>
    where
        Self: Sized;
}

/// The resolved meaning of a key chord: a built-in action, a config-defined
/// action id, or explicitly disabled (`"none"`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BindingTarget<A> {
    Core(A),
    Configured(String),
    Disabled,
}

impl<A: CoreAction> BindingTarget<A> {
    /// Parse a keybinding target: `"none"` disables, a known core action name
    /// maps to [`BindingTarget::Core`], and any other valid kebab-case id maps
    /// to [`BindingTarget::Configured`].
    pub fn parse(value: &str) -> Result<Self, String> {
        if value == "none" {
            return Ok(Self::Disabled);
        }
        if let Some(action) = A::parse(value) {
            return Ok(Self::Core(action));
        }
        if is_valid_action_id(value) {
            return Ok(Self::Configured(value.to_string()));
        }
        Err(format!("invalid action identifier: {value}"))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Core(action) => action.as_str(),
            Self::Configured(action) => action,
            Self::Disabled => "none",
        }
    }
}

/// Whether `value` is a valid custom action id: non-empty, lowercase ASCII
/// letters/digits/hyphens, no leading/trailing hyphen, no doubled hyphen.
pub fn is_valid_action_id(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.as_bytes().windows(2).any(|pair| pair == b"--")
}

/// A normalized key press: a [`KeyCode`] plus modifiers.
///
/// Construction normalizes so `Shift`+letter and `BackTab` compare equal to
/// their canonical forms regardless of how the terminal reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyChord {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let (code, modifiers) = normalize_chord(code, modifiers);
        Self { code, modifiers }
    }

    /// Parse a chord like `"ctrl+enter"` or `"control-option-shift-a"`.
    /// `+` and `-` are interchangeable separators.
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("key chord cannot be empty".to_string());
        }

        if value.chars().count() == 1 {
            return Ok(Self::new(
                KeyCode::Char(value.chars().next().expect("one character was checked")),
                KeyModifiers::NONE,
            ));
        }

        let normalized = value.replace('+', "-");
        let mut remaining = normalized.as_str();
        let mut modifiers = KeyModifiers::NONE;
        while let Some((prefix, rest)) = remaining.split_once('-') {
            let modifier = match prefix.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "option" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                "super" | "cmd" | "command" => KeyModifiers::SUPER,
                _ => break,
            };
            if modifiers.contains(modifier) {
                return Err(format!("duplicate key modifier: {prefix}"));
            }
            modifiers.insert(modifier);
            remaining = rest;
        }

        let code = parse_key_code(remaining)
            .ok_or_else(|| format!("invalid or unsupported key: {remaining}"))?;
        Ok(Self::new(code, modifiers))
    }

    /// Canonical `"ctrl+shift+enter"`-style rendering.
    pub fn as_str(self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("ctrl".to_string());
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("alt".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("shift".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SUPER) {
            parts.push("super".to_string());
        }
        parts.push(format_key_code(self.code));
        parts.join("+")
    }

    /// Whether `event` is a press/repeat matching this chord.
    pub fn matches_event(self, event: &KeyEvent) -> bool {
        matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && self == Self::new(event.code, event.modifiers)
    }
}

impl Ord for KeyChord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.code
            .partial_cmp(&other.code)
            .expect("crossterm key code ordering is total")
            .then_with(|| {
                self.modifiers
                    .partial_cmp(&other.modifiers)
                    .expect("crossterm modifier ordering is total")
            })
    }
}

impl PartialOrd for KeyChord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_str())
    }
}

fn normalize_chord(code: KeyCode, mut modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    match code {
        KeyCode::BackTab => {
            modifiers.insert(KeyModifiers::SHIFT);
            (KeyCode::Tab, modifiers)
        }
        KeyCode::Char(character) if character.is_ascii_uppercase() => {
            modifiers.insert(KeyModifiers::SHIFT);
            (KeyCode::Char(character.to_ascii_lowercase()), modifiers)
        }
        _ => (code, modifiers),
    }
}

/// Parse a single key name (letter, punctuation alias, named key, or `f1`..`f35`).
pub fn parse_key_code(value: &str) -> Option<KeyCode> {
    if value.chars().count() == 1 {
        return value.chars().next().map(KeyCode::Char);
    }

    let normalized = value.to_ascii_lowercase();
    let named = match normalized.as_str() {
        "enter" => KeyCode::Enter,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "esc" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "null" => KeyCode::Null,
        "caps-lock" => KeyCode::CapsLock,
        "scroll-lock" => KeyCode::ScrollLock,
        "num-lock" => KeyCode::NumLock,
        "print-screen" => KeyCode::PrintScreen,
        "pause" => KeyCode::Pause,
        "menu" => KeyCode::Menu,
        "keypad-begin" => KeyCode::KeypadBegin,
        "plus" => KeyCode::Char('+'),
        "equals" => KeyCode::Char('='),
        "colon" => KeyCode::Char(':'),
        "semicolon" => KeyCode::Char(';'),
        "comma" => KeyCode::Char(','),
        "period" => KeyCode::Char('.'),
        "minus" => KeyCode::Char('-'),
        "slash" => KeyCode::Char('/'),
        "backslash" => KeyCode::Char('\\'),
        "quote" => KeyCode::Char('\''),
        "backtick" => KeyCode::Char('`'),
        "left-bracket" => KeyCode::Char('['),
        "right-bracket" => KeyCode::Char(']'),
        _ => {
            if let Some(number) = normalized
                .strip_prefix('f')
                .and_then(|number| number.parse().ok())
            {
                if (1..=35).contains(&number) {
                    return Some(KeyCode::F(number));
                }
            }
            return None;
        }
    };
    Some(named)
}

/// Render a [`KeyCode`] back to its canonical name (inverse of
/// [`parse_key_code`]).
pub fn format_key_code(code: KeyCode) -> String {
    match code {
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "backtab".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::F(number) => format!("f{number}"),
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char('+') => "plus".to_string(),
        KeyCode::Char('=') => "equals".to_string(),
        KeyCode::Char(':') => "colon".to_string(),
        KeyCode::Char(';') => "semicolon".to_string(),
        KeyCode::Char(',') => "comma".to_string(),
        KeyCode::Char('.') => "period".to_string(),
        KeyCode::Char('-') => "minus".to_string(),
        KeyCode::Char('/') => "slash".to_string(),
        KeyCode::Char('\\') => "backslash".to_string(),
        KeyCode::Char('\'') => "quote".to_string(),
        KeyCode::Char('`') => "backtick".to_string(),
        KeyCode::Char('[') => "left-bracket".to_string(),
        KeyCode::Char(']') => "right-bracket".to_string(),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Null => "null".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::CapsLock => "caps-lock".to_string(),
        KeyCode::ScrollLock => "scroll-lock".to_string(),
        KeyCode::NumLock => "num-lock".to_string(),
        KeyCode::PrintScreen => "print-screen".to_string(),
        KeyCode::Pause => "pause".to_string(),
        KeyCode::Menu => "menu".to_string(),
        KeyCode::KeypadBegin => "keypad-begin".to_string(),
        KeyCode::Media(_) | KeyCode::Modifier(_) => "unsupported".to_string(),
    }
}

/// A single `chord → target` mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding<A> {
    pub chord: KeyChord,
    pub target: BindingTarget<A>,
}

impl<A> Binding<A> {
    pub const fn new(chord: KeyChord, target: BindingTarget<A>) -> Self {
        Self { chord, target }
    }
}

/// Context-scoped keybindings with layered overrides and fallback resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap<C, A> {
    bindings: BTreeMap<C, Vec<Binding<A>>>,
}

impl<C, A> Default for Keymap<C, A> {
    fn default() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }
}

impl<C: BindingContext, A: CoreAction> Keymap<C, A> {
    /// Insert `binding` into `context`, replacing any existing binding for the
    /// same chord while preserving insertion order.
    pub fn set(&mut self, context: C, binding: Binding<A>) {
        let bindings = self.bindings.entry(context).or_default();
        if let Some(existing) = bindings
            .iter_mut()
            .find(|existing| existing.chord == binding.chord)
        {
            *existing = binding;
        } else {
            bindings.push(binding);
        }
    }

    /// Remove every binding in `context` pointing at `target`.
    pub fn remove_target(&mut self, context: C, target: &BindingTarget<A>) {
        if let Some(bindings) = self.bindings.get_mut(&context) {
            bindings.retain(|binding| &binding.target != target);
        }
    }

    /// Overlay `layer` on top of this keymap (later layer wins per chord).
    pub fn apply_layer(&mut self, layer: &Self) {
        for (context, binding) in layer.iter() {
            self.set(context, binding.clone());
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (C, &Binding<A>)> {
        self.bindings
            .iter()
            .flat_map(|(&context, bindings)| bindings.iter().map(move |binding| (context, binding)))
    }

    /// Validate every binding target with `validate`, annotating errors with
    /// the context and chord.
    pub fn validate_targets(
        &self,
        mut validate: impl FnMut(C, &BindingTarget<A>) -> Result<(), String>,
    ) -> Result<(), String> {
        for (context, binding) in self.iter() {
            validate(context, &binding.target).map_err(|error| {
                format!(
                    "invalid keybinding {}.{}: {error}",
                    context.as_str(),
                    binding.chord
                )
            })?;
        }
        Ok(())
    }

    /// Resolve `event` in `context`, consulting the context's fallback chain.
    /// Release events never resolve.
    pub fn resolve(&self, context: C, event: &KeyEvent) -> Option<&BindingTarget<A>> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        for &candidate in context.fallback_contexts() {
            if let Some(binding) = self
                .bindings_for_context(candidate)
                .iter()
                .find(|binding| binding.chord.matches_event(event))
            {
                return Some(&binding.target);
            }
        }
        None
    }

    /// First chord bound to `target` reachable from `context`, skipping chords
    /// shadowed by a more specific context.
    pub fn first_chord_for_target(
        &self,
        context: C,
        target: &BindingTarget<A>,
    ) -> Option<KeyChord> {
        let contexts = context.fallback_contexts();
        for (index, &candidate) in contexts.iter().enumerate() {
            for binding in self.bindings_for_context(candidate) {
                let shadowed = contexts[..index].iter().any(|&more_specific| {
                    self.bindings_for_context(more_specific)
                        .iter()
                        .any(|other| other.chord == binding.chord)
                });
                if !shadowed && &binding.target == target {
                    return Some(binding.chord);
                }
            }
        }
        None
    }

    pub fn bindings_for_context(&self, context: C) -> &[Binding<A>] {
        self.bindings
            .get(&context)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestAction {
        Go,
    }

    impl CoreAction for TestAction {
        fn as_str(self) -> &'static str {
            "go"
        }
        fn parse(value: &str) -> Option<Self> {
            (value == "go").then_some(Self::Go)
        }
    }

    #[test]
    fn parses_and_formats_named_keys_and_function_keys() {
        assert_eq!(
            KeyChord::parse("ctrl+enter").unwrap(),
            KeyChord::new(KeyCode::Enter, KeyModifiers::CONTROL)
        );
        assert_eq!(KeyChord::parse("f5").unwrap().as_str(), "f5");
        assert_eq!(KeyChord::parse("space").unwrap().as_str(), "space");
        assert!(KeyChord::parse("f99").is_err());
    }

    #[test]
    fn uppercase_and_backtab_normalize_to_shift() {
        assert_eq!(
            KeyChord::new(KeyCode::Char('A'), KeyModifiers::NONE),
            KeyChord::new(KeyCode::Char('a'), KeyModifiers::SHIFT)
        );
        assert_eq!(
            KeyChord::new(KeyCode::BackTab, KeyModifiers::NONE),
            KeyChord::new(KeyCode::Tab, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn modifier_aliases_are_accepted() {
        assert_eq!(
            KeyChord::parse("control-option-shift-command-a").unwrap(),
            KeyChord::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL
                    | KeyModifiers::ALT
                    | KeyModifiers::SHIFT
                    | KeyModifiers::SUPER
            )
        );
    }

    #[test]
    fn binding_targets_validate_identifiers() {
        assert_eq!(
            BindingTarget::<TestAction>::parse("none").unwrap(),
            BindingTarget::Disabled
        );
        assert_eq!(
            BindingTarget::<TestAction>::parse("go").unwrap(),
            BindingTarget::Core(TestAction::Go)
        );
        assert_eq!(
            BindingTarget::<TestAction>::parse("open-thing").unwrap(),
            BindingTarget::Configured("open-thing".to_string())
        );
        assert!(BindingTarget::<TestAction>::parse("-bad").is_err());
        assert!(BindingTarget::<TestAction>::parse("bad--id").is_err());
    }

    #[test]
    fn validates_action_ids() {
        assert!(is_valid_action_id("open-thing"));
        assert!(!is_valid_action_id(""));
        assert!(!is_valid_action_id("-x"));
        assert!(!is_valid_action_id("x-"));
        assert!(!is_valid_action_id("a--b"));
        assert!(!is_valid_action_id("Upper"));
    }
}
