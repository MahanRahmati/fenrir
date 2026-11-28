use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

pub fn wrap(
  segments: Vec<(String, Style)>,
  width: usize,
) -> Vec<Line<'static>> {
  if width == 0 {
    return vec![line_from(segments)];
  }
  let mut lines = Vec::new();
  let mut current: Vec<(String, Style)> = Vec::new();
  let mut current_width = 0;
  let mut pending_space: Option<Style> = None;
  for token in tokenize(segments) {
    match token {
      Token::Newline => {
        lines.push(line_from(std::mem::take(&mut current)));
        current_width = 0;
        pending_space = None;
      }
      Token::Space(style) => pending_space = Some(style),
      Token::Word(word) => {
        let word_width = word.width;
        let space_width =
          usize::from(pending_space.is_some() && current_width > 0);
        if current_width > 0 && current_width + space_width + word_width > width
        {
          lines.push(line_from(std::mem::take(&mut current)));
          current_width = 0;
          pending_space = None;
        }
        if let Some(style) = pending_space.take()
          && current_width > 0
        {
          current.push((" ".to_string(), style));
          current_width += 1;
        }
        if word_width <= width {
          for (text, style) in word.segments {
            current.push((text, style));
          }
          current_width += word_width;
        } else {
          for (text, style) in word.segments {
            for ch in text.chars() {
              let char_width = char_width(ch);
              if current_width + char_width > width && current_width > 0 {
                lines.push(line_from(std::mem::take(&mut current)));
                current_width = 0;
              }
              current.push((ch.to_string(), style));
              current_width += char_width;
            }
          }
        }
      }
    }
  }
  if !current.is_empty() || lines.is_empty() {
    lines.push(line_from(current));
  }
  return lines;
}

struct Word {
  segments: Vec<(String, Style)>,
  width: usize,
}

impl Word {
  fn new() -> Self {
    return Self {
      segments: Vec::new(),
      width: 0,
    };
  }

  fn push(&mut self, ch: char, style: Style) {
    self.width += char_width(ch);
    if let Some((text, last_style)) = self.segments.last_mut()
      && *last_style == style
    {
      text.push(ch);
      return;
    }
    self.segments.push((ch.to_string(), style));
  }
}

enum Token {
  Word(Word),
  Space(Style),
  Newline,
}

fn tokenize(segments: Vec<(String, Style)>) -> Vec<Token> {
  let mut tokens = Vec::new();
  let mut word: Option<Word> = None;
  let mut space: Option<Style> = None;
  for (text, style) in segments {
    for ch in text.chars() {
      if ch == '\n' {
        flush_word(&mut word, &mut tokens);
        flush_space(&mut space, &mut tokens);
        tokens.push(Token::Newline);
      } else if ch.is_whitespace() {
        flush_word(&mut word, &mut tokens);
        space = Some(without_underline(style));
      } else {
        flush_space(&mut space, &mut tokens);
        word.get_or_insert_with(Word::new).push(ch, style);
      }
    }
  }
  flush_word(&mut word, &mut tokens);
  flush_space(&mut space, &mut tokens);
  return tokens;
}

fn flush_word(word: &mut Option<Word>, tokens: &mut Vec<Token>) {
  if let Some(word) = word.take() {
    tokens.push(Token::Word(word));
  }
}

fn flush_space(space: &mut Option<Style>, tokens: &mut Vec<Token>) {
  if let Some(style) = space.take() {
    tokens.push(Token::Space(style));
  }
}

fn without_underline(mut style: Style) -> Style {
  style.add_modifier = style.add_modifier.difference(Modifier::UNDERLINED);
  return style;
}

fn line_from(mut segments: Vec<(String, Style)>) -> Line<'static> {
  while segments
    .last()
    .is_some_and(|(text, _)| text.trim_end().is_empty())
  {
    segments.pop();
  }
  if let Some((text, _)) = segments.last_mut() {
    let trimmed_len = text.trim_end().len();
    text.truncate(trimmed_len);
  }
  return Line::from(spans(segments));
}

fn spans(segments: Vec<(String, Style)>) -> Vec<Span<'static>> {
  let mut spans: Vec<Span<'static>> = Vec::new();
  for (text, style) in segments {
    if text.is_empty() {
      continue;
    }
    if let Some(last) = spans.last_mut()
      && last.style == style
    {
      last.content.to_mut().push_str(&text);
      continue;
    }
    spans.push(Span::styled(text, style));
  }
  return spans;
}

pub fn width(text: &str) -> usize {
  return text.chars().map(char_width).sum();
}

pub fn char_width(ch: char) -> usize {
  return UnicodeWidthChar::width(ch).unwrap_or(0);
}
