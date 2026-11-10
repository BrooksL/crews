//! Short key labels for app-style hints (`ui.style = "app"`), read from the
//! live keybinds: `ctrl+b` reads `⌃B`, and a prefix binding reads `⌃B q`.

use crate::config::{ActionKeybinds, LiveKeybindConfig};

/// The key or keys that run an action, or `None` when it's unbound. Prefix
/// bindings are preferred; when an action has several, the first is shown.
pub(super) fn action_hint(keybinds: &LiveKeybindConfig, action: &ActionKeybinds) -> Option<String> {
    if let Some(rhs) = action.prefix_rhs_label() {
        let first = rhs.split(" / ").next().unwrap_or(&rhs);
        return Some(format!(
            "{} {}",
            compact_key(&keybinds.primary_prefix_label()),
            compact_key(first)
        ));
    }
    action.labels().first().map(|label| compact_key(label))
}

/// `ctrl+shift+t` → `⌃⇧T`. Keys without modifiers keep their case.
pub(super) fn compact_key(label: &str) -> String {
    let mut parts = label.split('+').collect::<Vec<_>>();
    // `ctrl++` names the plus key.
    if label.ends_with("++") {
        parts.truncate(parts.len().saturating_sub(2));
        parts.push("+");
    }
    let Some((key, modifiers)) = parts.split_last() else {
        return label.to_owned();
    };
    let mut text = String::new();
    for modifier in modifiers {
        text.push_str(match *modifier {
            "ctrl" => "⌃",
            "alt" => "⌥",
            "shift" => "⇧",
            "cmd" | "super" => "⌘",
            other => other,
        });
    }
    if !modifiers.is_empty() && key.chars().count() == 1 {
        text.push_str(&key.to_uppercase());
    } else {
        text.push_str(key);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_keys_use_modifier_symbols() {
        assert_eq!(compact_key("ctrl+b"), "⌃B");
        assert_eq!(compact_key("ctrl+shift+t"), "⌃⇧T");
        assert_eq!(compact_key("alt+enter"), "⌥enter");
        assert_eq!(compact_key("cmd+k"), "⌘K");
        assert_eq!(compact_key("q"), "q");
        assert_eq!(compact_key("ctrl++"), "⌃+");
    }

    #[test]
    fn default_detach_reads_prefix_then_key() {
        let config =
            super::super::ClientShellConfig::from_config(&crate::config::Config::default());
        let keybinds = &config.keybinds;
        assert_eq!(
            action_hint(keybinds, &keybinds.keybinds.detach).as_deref(),
            Some("⌃B q")
        );
    }
}
