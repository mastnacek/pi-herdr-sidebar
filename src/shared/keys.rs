//! Shared key-event classification (used by every text input in the app).
//!
//! On Windows, a Czech/Slovak keyboard produces `@`, `#`, `\`, … through
//! **AltGr**, which crossterm reports as `CONTROL | ALT` together with the
//! *layout-produced* character (e.g. AltGr+V → `Char('@')`). Treating any
//! CONTROL as a shortcut therefore silently swallows whole classes of
//! characters. The rule here: plain Ctrl (no Alt) is a shortcut, AltGr
//! (Ctrl+Alt) is text — our shortcuts never combine Ctrl with Alt.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Is this key event an ordinary text input (should land in a buffer)?
///
/// - no CONTROL (plain or Shift, any ALT) → text;
/// - CONTROL **and** ALT together → AltGr text (the char is what the layout
///   produced, not the pressed key);
/// - CONTROL alone → a Ctrl shortcut (Ctrl+S, Ctrl+D, …), never text.
pub fn is_text_input(key: &KeyEvent) -> bool {
    if !matches!(key.code, KeyCode::Char(_)) {
        return false;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match (ctrl, alt) {
        (false, _) => true,
        (true, true) => true, // AltGr
        (true, false) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers as M;

    fn ev(c: char, m: M) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), m)
    }

    #[test]
    fn plain_and_shift_chars_are_text() {
        assert!(is_text_input(&ev('a', M::NONE)));
        assert!(is_text_input(&ev('A', M::SHIFT)));
        assert!(is_text_input(&ev(' ', M::NONE)));
    }

    #[test]
    fn altgr_chars_are_text_even_with_ctrl() {
        // Czech keyboard: '@' arrives as Ctrl+Alt+V in crossterm terms.
        assert!(is_text_input(&ev('@', M::CONTROL | M::ALT)));
        assert!(is_text_input(&ev('\\', M::CONTROL | M::ALT)));
        assert!(is_text_input(&ev('#', M::CONTROL | M::ALT | M::SHIFT)));
    }

    #[test]
    fn plain_ctrl_chars_are_shortcuts_not_text() {
        assert!(!is_text_input(&ev('s', M::CONTROL)));
        assert!(!is_text_input(&ev('d', M::CONTROL)));
    }

    #[test]
    fn non_char_events_are_never_text() {
        let enter = KeyEvent::new(KeyCode::Enter, M::NONE);
        assert!(!is_text_input(&enter));
    }
}
