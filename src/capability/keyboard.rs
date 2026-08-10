use crate::capability::errors::*;
use crate::utils::process;

pub async fn type_with_keyboard(text: &str) -> CapabilityErrorResult<String> {
  let output = process::run("ydotool", &["type", text])
    .await
    .map_err(|e| CapabilityError::Keyboard {
      reason: format!("Failed to run ydotool: {e}"),
    })?;
  if !output.exit_code.is_success() {
    return Err(CapabilityError::Keyboard {
      reason: ydotool_failure(&output, "ydotool could not type the text"),
    });
  }
  return Ok(format!("Typed text ({} characters)", text.chars().count()));
}

pub async fn press_key_on_keyboard(key: &str) -> CapabilityErrorResult<String> {
  let codes = key_combination(key)?;
  let args: Vec<String> = codes
    .iter()
    .map(|code| format!("{code}:1"))
    .chain(codes.iter().rev().map(|code| format!("{code}:0")))
    .collect();
  let mut cmd = vec!["key"];
  cmd.extend(args.iter().map(|arg| arg.as_str()));
  let output = process::run("ydotool", &cmd).await.map_err(|e| {
    CapabilityError::Keyboard {
      reason: format!("Failed to run ydotool: {e}"),
    }
  })?;
  if !output.exit_code.is_success() {
    return Err(CapabilityError::Keyboard {
      reason: ydotool_failure(&output, "ydotool could not press the key"),
    });
  }
  return Ok(format!("Pressed '{key}'"));
}

fn key_combination(key: &str) -> CapabilityErrorResult<Vec<i32>> {
  let mut codes = Vec::new();
  for part in key.split('+').map(|p| p.trim().to_ascii_lowercase()) {
    if part.is_empty() {
      continue;
    }
    if let Some(key) = Key::from_name(&part) {
      codes.push(key.evdev_code());
      continue;
    }
    let mut chars = part.chars();
    let c = chars.next();
    if chars.next().is_some() {
      return Err(CapabilityError::Keyboard {
        reason: format!("Unknown key '{part}'"),
      });
    }
    let Some(c) = c else {
      return Err(CapabilityError::Keyboard {
        reason: format!("Cannot parse key '{key}'"),
      });
    };
    let Some((code, shifted)) = char_evdev_code(c) else {
      return Err(CapabilityError::Keyboard {
        reason: format!("Unknown key '{part}'"),
      });
    };
    if shifted {
      codes.push(Key::Shift.evdev_code());
    }
    codes.push(code);
  }
  if codes.is_empty() {
    return Err(CapabilityError::Keyboard {
      reason: format!("Cannot parse key '{key}'"),
    });
  }
  return Ok(codes);
}

fn ydotool_failure(output: &process::Output, fallback: &str) -> String {
  let stderr = output.stderr.trim();
  if stderr.is_empty() {
    return fallback.to_string();
  }
  let mut reason = stderr.to_string();
  if stderr.contains("check if ydotoold is running") {
    reason.push_str(" Start it with: sudo -b ydotoold --socket-own=1000:1000");
  }
  return reason;
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Key {
  Enter,
  Escape,
  Backspace,
  Tab,
  Space,
  Left,
  Up,
  Right,
  Down,
  Home,
  End,
  PageUp,
  PageDown,
  Insert,
  Delete,
  CapsLock,
  NumLock,
  ScrollLock,
  F1,
  F2,
  F3,
  F4,
  F5,
  F6,
  F7,
  F8,
  F9,
  F10,
  F11,
  F12,
  Control,
  Shift,
  Alt,
  Super,
}

impl Key {
  fn evdev_code(&self) -> i32 {
    return match self {
      Self::Enter => 28,
      Self::Escape => 1,
      Self::Backspace => 14,
      Self::Tab => 15,
      Self::Space => 57,
      Self::Left => 105,
      Self::Up => 103,
      Self::Right => 106,
      Self::Down => 108,
      Self::Home => 102,
      Self::End => 107,
      Self::PageUp => 104,
      Self::PageDown => 109,
      Self::Insert => 110,
      Self::Delete => 111,
      Self::CapsLock => 58,
      Self::NumLock => 69,
      Self::ScrollLock => 70,
      Self::F1 => 59,
      Self::F2 => 60,
      Self::F3 => 61,
      Self::F4 => 62,
      Self::F5 => 63,
      Self::F6 => 64,
      Self::F7 => 65,
      Self::F8 => 66,
      Self::F9 => 67,
      Self::F10 => 68,
      Self::F11 => 87,
      Self::F12 => 88,
      Self::Control => 29,
      Self::Shift => 42,
      Self::Alt => 56,
      Self::Super => 125,
    };
  }

  fn from_name(name: &str) -> Option<Self> {
    return match name.to_ascii_lowercase().as_str() {
      "enter" | "return" => Some(Self::Enter),
      "escape" | "esc" => Some(Self::Escape),
      "backspace" => Some(Self::Backspace),
      "tab" => Some(Self::Tab),
      "space" => Some(Self::Space),
      "left" => Some(Self::Left),
      "up" => Some(Self::Up),
      "right" => Some(Self::Right),
      "down" => Some(Self::Down),
      "home" => Some(Self::Home),
      "end" => Some(Self::End),
      "pageup" | "prior" => Some(Self::PageUp),
      "pagedown" | "next" => Some(Self::PageDown),
      "insert" => Some(Self::Insert),
      "delete" => Some(Self::Delete),
      "capslock" => Some(Self::CapsLock),
      "numlock" => Some(Self::NumLock),
      "scrolllock" => Some(Self::ScrollLock),
      "f1" => Some(Self::F1),
      "f2" => Some(Self::F2),
      "f3" => Some(Self::F3),
      "f4" => Some(Self::F4),
      "f5" => Some(Self::F5),
      "f6" => Some(Self::F6),
      "f7" => Some(Self::F7),
      "f8" => Some(Self::F8),
      "f9" => Some(Self::F9),
      "f10" => Some(Self::F10),
      "f11" => Some(Self::F11),
      "f12" => Some(Self::F12),
      "ctrl" | "control" | "control_l" => Some(Self::Control),
      "shift" | "shift_l" => Some(Self::Shift),
      "alt" | "alt_l" => Some(Self::Alt),
      "super" | "meta" | "win" | "super_l" => Some(Self::Super),
      _ => None,
    };
  }
}

fn char_evdev_code(c: char) -> Option<(i32, bool)> {
  return match c {
    'a'..='z' => Some((letter_evdev_code(c)?, false)),
    'A'..='Z' => Some((letter_evdev_code(c)?, true)),
    '1'..='9' => Some((2 + (c as i32 - '1' as i32), false)),
    '0' => Some((11, false)),
    ' ' => Some((57, false)),
    '-' => Some((12, false)),
    '=' => Some((13, false)),
    '[' => Some((26, false)),
    ']' => Some((27, false)),
    '\\' => Some((43, false)),
    ';' => Some((39, false)),
    '\'' => Some((40, false)),
    '`' => Some((41, false)),
    ',' => Some((51, false)),
    '.' => Some((52, false)),
    '/' => Some((53, false)),
    '~' => Some((41, true)),
    '!' => Some((2, true)),
    '@' => Some((3, true)),
    '#' => Some((4, true)),
    '$' => Some((5, true)),
    '%' => Some((6, true)),
    '^' => Some((7, true)),
    '&' => Some((8, true)),
    '*' => Some((9, true)),
    '(' => Some((10, true)),
    ')' => Some((11, true)),
    '_' => Some((12, true)),
    '+' => Some((13, true)),
    '{' => Some((26, true)),
    '}' => Some((27, true)),
    '|' => Some((43, true)),
    ':' => Some((39, true)),
    '"' => Some((40, true)),
    '<' => Some((51, true)),
    '>' => Some((52, true)),
    '?' => Some((53, true)),
    _ => None,
  };
}

fn letter_evdev_code(c: char) -> Option<i32> {
  return match c.to_ascii_lowercase() {
    'a' => Some(30),
    'b' => Some(48),
    'c' => Some(46),
    'd' => Some(32),
    'e' => Some(18),
    'f' => Some(33),
    'g' => Some(34),
    'h' => Some(35),
    'i' => Some(23),
    'j' => Some(36),
    'k' => Some(37),
    'l' => Some(38),
    'm' => Some(50),
    'n' => Some(49),
    'o' => Some(24),
    'p' => Some(25),
    'q' => Some(16),
    'r' => Some(19),
    's' => Some(31),
    't' => Some(20),
    'u' => Some(22),
    'v' => Some(47),
    'w' => Some(17),
    'x' => Some(45),
    'y' => Some(21),
    'z' => Some(44),
    _ => None,
  };
}
