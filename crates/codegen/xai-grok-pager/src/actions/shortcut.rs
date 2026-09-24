//! Parse and apply `[ui].copy_markdown_shortcut`.
//!
//! The compiled default is Ctrl+Shift+Y, with F6 as a second chord for terminals that cannot
//! deliver Ctrl+Shift+letter. A configured value replaces both. `off` unbinds the action.

use crossterm::event::{KeyCode, KeyModifiers};

use super::{ActionId, ActionRegistry};
use crate::input::key::KeyShortcut;
use crate::key;

/// Shown in Settings and accepted (any case) as "restore the compiled default".
pub const COPY_MARKDOWN_SHORTCUT_DEFAULT: &str = "Ctrl+Shift+y";

/// What a config / settings value means for the copy-markdown chord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyMarkdownBinding {
    /// Ctrl+Shift+Y and F6.
    Default,
    /// No key. The action stays available from the command path that calls it directly.
    Off,
    /// One chord, replacing the compiled default and its alt.
    Custom(KeyShortcut),
}

/// `None`, empty, and the default label keep the compiled chords.
/// `off` / `none` / `disabled` / `unbound` clear them.
pub fn resolve_copy_markdown_spec(spec: Option<&str>) -> Result<CopyMarkdownBinding, String> {
    let Some(spec) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(CopyMarkdownBinding::Default);
    };
    if spec.eq_ignore_ascii_case(COPY_MARKDOWN_SHORTCUT_DEFAULT)
        || spec.eq_ignore_ascii_case("default")
    {
        return Ok(CopyMarkdownBinding::Default);
    }
    if matches!(
        spec.to_ascii_lowercase().as_str(),
        "off" | "none" | "disabled" | "unbound"
    ) {
        return Ok(CopyMarkdownBinding::Off);
    }
    let key = parse_shortcut(spec)?;
    // Any spelling of the compiled primary chord keeps F6 too. A different chord replaces both.
    if key == default_copy_markdown_keys().0 {
        return Ok(CopyMarkdownBinding::Default);
    }
    if is_typable(&key) {
        return Err(
            "use a chord with Ctrl, Alt, or Super, or a function key. Plain letters are typed into the prompt"
                .to_string(),
        );
    }
    if prompt_editor_claims(&key) {
        return Err(format!(
            "{} is used by the prompt editor",
            key.display_pretty()
        ));
    }
    Ok(CopyMarkdownBinding::Custom(key))
}

impl ActionRegistry {
    /// Install `spec` on [`ActionId::CopyMarkdownSource`].
    /// `None` restores the compiled default. A chord another action already owns is rejected and
    /// the registry is left unchanged.
    pub fn apply_copy_markdown_shortcut(&mut self, spec: Option<&str>) -> Result<(), String> {
        let binding = resolve_copy_markdown_spec(spec)?;
        let (key, alts) = match &binding {
            CopyMarkdownBinding::Default => default_copy_markdown_keys(),
            CopyMarkdownBinding::Off => (key!(Null), Vec::new()),
            CopyMarkdownBinding::Custom(key) => (*key, Vec::new()),
        };
        if key.code != KeyCode::Null {
            for candidate in std::iter::once(key).chain(alts.iter().copied()) {
                if let Some(owner) = self.binding_owner(candidate, ActionId::CopyMarkdownSource) {
                    return Err(format!(
                        "{} is already bound to {owner}",
                        candidate.display_pretty()
                    ));
                }
            }
        }
        if let Some(def) = self
            .actions
            .iter_mut()
            .find(|d| d.id == ActionId::CopyMarkdownSource)
        {
            def.default_key = key;
            def.alt_keys = alts;
        }
        Ok(())
    }

    fn binding_owner(&self, key: KeyShortcut, except: ActionId) -> Option<&'static str> {
        self.actions
            .iter()
            .find(|def| def.id != except && (def.default_key == key || def.alt_keys.contains(&key)))
            .map(|def| def.description)
    }
}

fn default_copy_markdown_keys() -> (KeyShortcut, Vec<KeyShortcut>) {
    (key!('y', CONTROL | SHIFT), vec![key!(F(6))])
}

/// Last `+` segment is the key; earlier segments are modifiers, in any order.
fn parse_shortcut(spec: &str) -> Result<KeyShortcut, String> {
    let parts: Vec<&str> = spec
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let Some((key_token, modifiers)) = parts.split_last() else {
        return Err("shortcut is empty".to_string());
    };
    let mut mods = KeyModifiers::NONE;
    for modifier in modifiers {
        mods |= match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "ctl" => KeyModifiers::CONTROL,
            "alt" | "opt" | "option" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            "cmd" | "command" | "super" | "win" | "meta" => KeyModifiers::SUPER,
            other => {
                return Err(format!(
                    "unknown modifier \"{other}\". Use ctrl, alt, shift, or super, then + and a key"
                ));
            }
        };
    }
    let code = parse_key_token(key_token)?;
    if code == KeyCode::Null {
        return Err("that key cannot be bound. Use off to unbind".to_string());
    }
    Ok(KeyShortcut::new(code, mods))
}

fn parse_key_token(token: &str) -> Result<KeyCode, String> {
    let lower = token.to_ascii_lowercase();
    if lower.chars().count() == 1 {
        let ch = lower.chars().next().expect("len 1");
        if ch.is_control() {
            return Err(format!("unsupported key \"{token}\""));
        }
        return Ok(KeyCode::Char(ch));
    }
    if let Some(rest) = lower.strip_prefix('f')
        && let Ok(n) = rest.parse::<u8>()
        && (1..=12).contains(&n)
    {
        return Ok(KeyCode::F(n));
    }
    let code = match lower.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "backspace" | "bsp" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        _ => {
            return Err(format!(
                "unknown key \"{token}\". Examples: y, f6, enter, tab, up"
            ));
        }
    };
    Ok(code)
}

/// A key the prompt would insert as text, so it can never be an agent-screen chord.
fn is_typable(key: &KeyShortcut) -> bool {
    if key.is_letter_or_shift_letter() {
        return true;
    }
    let KeyCode::Char(ch) = key.code else {
        return false;
    };
    if ch.is_control() {
        return false;
    }
    let mods = key.modifiers - KeyModifiers::SHIFT;
    mods.is_empty()
}

/// Chords the prompt editor consumes before (or instead of) an agent action.
/// Registry collisions are checked separately; this list is the editor's own readline set.
fn prompt_editor_claims(key: &KeyShortcut) -> bool {
    const LETTERS: &[char] = &[
        'a', 'b', 'd', 'e', 'f', 'h', 'k', 'n', 'p', 'u', 'v', 'w', 'y',
    ];
    if key.modifiers == KeyModifiers::CONTROL
        && let KeyCode::Char(ch) = key.code
        && LETTERS.contains(&ch.to_ascii_lowercase())
    {
        return true;
    }
    let claimed = [
        key!('z', CONTROL),
        key!('z', CONTROL | SHIFT),
        key!('z', ALT),
        key!('b', ALT),
        key!('f', ALT),
        key!(Left, CONTROL),
        key!(Right, CONTROL),
        key!(Left, ALT),
        key!(Right, ALT),
        key!(Left, SUPER),
        key!(Right, SUPER),
        key!('c', SUPER),
        key!('x', SUPER),
        key!('x', CONTROL),
    ];
    claimed.contains(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::When;
    use crate::key;
    use crossterm::event::KeyEvent;

    #[test]
    fn default_label_matches_the_compiled_chord() {
        let (primary, alts) = default_copy_markdown_keys();
        assert_eq!(primary.display_pretty(), COPY_MARKDOWN_SHORTCUT_DEFAULT);
        assert_eq!(alts, vec![key!(F(6))]);
    }

    #[test]
    fn parses_ctrl_shift_y_in_any_modifier_order_and_case() {
        let expected = key!('y', CONTROL | SHIFT);
        assert_eq!(
            resolve_copy_markdown_spec(Some("ctrl+shift+y")).unwrap(),
            CopyMarkdownBinding::Default
        );
        assert_eq!(
            resolve_copy_markdown_spec(Some("SHIFT+CTRL+Y")).unwrap(),
            CopyMarkdownBinding::Default
        );
        assert_eq!(
            parse_shortcut("shift+ctrl+y").unwrap(),
            expected,
            "a non-canonical spelling still parses to the same chord"
        );
    }

    #[test]
    fn off_and_empty_and_custom_function_key() {
        assert_eq!(
            resolve_copy_markdown_spec(None).unwrap(),
            CopyMarkdownBinding::Default
        );
        assert_eq!(
            resolve_copy_markdown_spec(Some("  ")).unwrap(),
            CopyMarkdownBinding::Default
        );
        assert_eq!(
            resolve_copy_markdown_spec(Some("OFF")).unwrap(),
            CopyMarkdownBinding::Off
        );
        assert_eq!(
            resolve_copy_markdown_spec(Some("f6")).unwrap(),
            CopyMarkdownBinding::Custom(key!(F(6)))
        );
        assert_eq!(
            resolve_copy_markdown_spec(Some("alt+y")).unwrap(),
            CopyMarkdownBinding::Custom(key!('y', ALT))
        );
    }

    #[test]
    fn rejects_typed_letters_editor_chords_and_unknown_tokens() {
        assert!(resolve_copy_markdown_spec(Some("y")).is_err());
        assert!(resolve_copy_markdown_spec(Some("shift+y")).is_err());
        assert!(resolve_copy_markdown_spec(Some("ctrl+a")).is_err());
        assert!(resolve_copy_markdown_spec(Some("ctrl+y")).is_err());
        assert!(resolve_copy_markdown_spec(Some("ctrl+nope")).is_err());
        assert!(resolve_copy_markdown_spec(Some("hyper+y")).is_err());
    }

    #[test]
    fn apply_replaces_both_default_chords_and_rejects_conflicts() {
        let mut registry = ActionRegistry::defaults();
        let ctrl_shift_y = key!('y', CONTROL | SHIFT).to_key_event();
        let f6 = key!(F(6)).to_key_event();
        assert_eq!(
            registry.lookup(&ctrl_shift_y, When::AgentScreen),
            Some(ActionId::CopyMarkdownSource)
        );
        assert_eq!(
            registry.lookup(&f6, When::AgentScreen),
            Some(ActionId::CopyMarkdownSource)
        );

        registry
            .apply_copy_markdown_shortcut(Some("alt+y"))
            .unwrap();
        assert_eq!(
            registry.lookup(&key!('y', ALT).to_key_event(), When::AgentScreen),
            Some(ActionId::CopyMarkdownSource)
        );
        assert_eq!(
            registry.lookup(&ctrl_shift_y, When::AgentScreen),
            None,
            "a custom chord replaces the compiled default"
        );
        assert_eq!(registry.lookup(&f6, When::AgentScreen), None);

        let err = registry
            .apply_copy_markdown_shortcut(Some("ctrl+c"))
            .unwrap_err();
        assert!(err.contains("Cancel"), "{err}");
        assert_eq!(
            registry.lookup(&key!('y', ALT).to_key_event(), When::AgentScreen),
            Some(ActionId::CopyMarkdownSource),
            "a rejected chord must leave the previous binding in place"
        );

        registry.apply_copy_markdown_shortcut(Some("off")).unwrap();
        assert_eq!(
            registry.lookup(&key!('y', ALT).to_key_event(), When::AgentScreen),
            None
        );
        assert_eq!(
            registry.lookup(
                &KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE),
                When::AgentScreen
            ),
            None
        );

        registry.apply_copy_markdown_shortcut(None).unwrap();
        assert_eq!(
            registry.lookup(&ctrl_shift_y, When::AgentScreen),
            Some(ActionId::CopyMarkdownSource)
        );
    }
}
