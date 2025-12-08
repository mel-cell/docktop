use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn key_matches(event: KeyEvent, binding: &str) -> bool {
    if let Some((code, modifiers)) = parse_key(binding) {
        // We check if the event modifiers contain the required modifiers.
        // We might want exact match, but usually "contains" is safer for simple apps,
        // unless we want to distinguish "Ctrl+c" from "Ctrl+Shift+c".
        // For now let's enforce exact modifier match for Control/Alt, but maybe be lenient on Shift if it's a char?
        // Actually, let's just check if the required modifiers are present.
        
        // Special case: if binding is just a char (e.g. 'q'), we usually don't care if Shift is held (unless it's 'Q').
        // But crossterm handles 'q' vs 'Q'.
        
        if event.code == code {
             // If modifiers are specified in binding, they must match.
             // If no modifiers in binding, we generally ignore extra modifiers unless it's a special key.
             if modifiers.is_empty() {
                 return true;
             }
             return event.modifiers.contains(modifiers);
        }
    }
    false
}

pub fn parse_key(binding: &str) -> Option<(KeyCode, KeyModifiers)> {
    let binding = binding.to_lowercase();
    let parts: Vec<&str> = binding.split('+').collect();
    
    let mut modifiers = KeyModifiers::empty();
    
    // If there is only one part, it's just the key code
    let code_str = if parts.len() > 1 {
        for part in parts.iter().take(parts.len() - 1) {
            match *part {
                "ctrl" => modifiers.insert(KeyModifiers::CONTROL),
                "alt" => modifiers.insert(KeyModifiers::ALT),
                "shift" => modifiers.insert(KeyModifiers::SHIFT),
                _ => {}
            }
        }
        parts.last()?
    } else {
        parts[0]
    };

    let code = match code_str {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
        _ => return None,
    };

    Some((code, modifiers))
}
