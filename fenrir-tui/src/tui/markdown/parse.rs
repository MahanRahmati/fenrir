use pulldown_cmark::{
  Alignment as MarkdownAlignment, CodeBlockKind, Event, HeadingLevel, Options,
  Parser, Tag, TagEnd,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
  Text(String),
  Code(String),
  Strong(Vec<Inline>),
  Emphasis(Vec<Inline>),
  Strikethrough(Vec<Inline>),
  Link(Vec<Inline>),
  SoftBreak,
  HardBreak,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
  Heading { level: u8, inlines: Vec<Inline> },
  Paragraph(Vec<Inline>),
  Code { lang: Option<String>, text: String },
  List(List),
  Table(Table),
  Quote(Vec<Block>),
  Rule,
  Blank,
}

#[derive(Clone, Debug, PartialEq)]
pub struct List {
  pub ordered: bool,
  pub start: u64,
  pub items: Vec<ListItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
  pub task: Option<bool>,
  pub blocks: Vec<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Alignment {
  None,
  Left,
  Center,
  Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
  pub alignments: Vec<Alignment>,
  pub header: Vec<Vec<Inline>>,
  pub rows: Vec<Vec<Vec<Inline>>>,
}

struct TableState {
  alignments: Vec<Alignment>,
  header: Vec<Vec<Inline>>,
  rows: Vec<Vec<Vec<Inline>>>,
  current_row: Vec<Vec<Inline>>,
}

enum Container {
  Root(Vec<Block>),
  Quote(Vec<Block>),
  List(List),
  Item(ListItem),
  Table(TableState),
}

pub fn blocks(markdown: &str) -> Vec<Block> {
  let options = Options::ENABLE_TABLES
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_TASKLISTS;
  let parser = Parser::new_ext(markdown, options);
  let mut containers: Vec<Container> = vec![Container::Root(Vec::new())];
  let mut inline: Vec<Vec<Inline>> = vec![Vec::new()];
  let mut heading: Option<u8> = None;
  let mut code: Option<(Option<String>, String)> = None;
  for event in parser {
    match event {
      Event::Start(Tag::Paragraph) => inline.push(Vec::new()),
      Event::End(TagEnd::Paragraph) => {
        let inlines = inline.pop().unwrap_or_default();
        push_paragraph(&mut containers, inlines);
      }
      Event::Start(Tag::Heading { level, .. }) => {
        inline.push(Vec::new());
        heading = Some(heading_level(level));
      }
      Event::End(TagEnd::Heading(_)) => {
        let level = heading.take().unwrap_or(1);
        let inlines = inline.pop().unwrap_or_default();
        if !inlines.is_empty() {
          push_block(&mut containers, Block::Heading { level, inlines });
          blank_line(&mut containers);
        }
      }
      Event::Start(Tag::Strong) => inline.push(Vec::new()),
      Event::End(TagEnd::Strong) => wrap_inline(&mut inline, Inline::Strong),
      Event::Start(Tag::Emphasis) => inline.push(Vec::new()),
      Event::End(TagEnd::Emphasis) => {
        wrap_inline(&mut inline, Inline::Emphasis)
      }
      Event::Start(Tag::Strikethrough) => inline.push(Vec::new()),
      Event::End(TagEnd::Strikethrough) => {
        wrap_inline(&mut inline, Inline::Strikethrough);
      }
      Event::Start(Tag::Link { .. }) => inline.push(Vec::new()),
      Event::End(TagEnd::Link) => wrap_inline(&mut inline, Inline::Link),
      Event::Start(Tag::CodeBlock(kind)) => {
        flush_inline(&mut inline, &mut containers);
        code = Some((code_language(kind), String::new()));
      }
      Event::End(TagEnd::CodeBlock) => {
        if let Some((lang, text)) = code.take() {
          push_block(&mut containers, Block::Code { lang, text });
          blank_line(&mut containers);
        }
      }
      Event::Start(Tag::BlockQuote(_)) => {
        flush_inline(&mut inline, &mut containers);
        containers.push(Container::Quote(Vec::new()));
      }
      Event::End(TagEnd::BlockQuote(_)) => {
        flush_inline(&mut inline, &mut containers);
        let Some(Container::Quote(mut blocks)) = containers.pop() else {
          continue;
        };
        trim_blank_edges(&mut blocks);
        push_block(&mut containers, Block::Quote(blocks));
        blank_line(&mut containers);
      }
      Event::Start(Tag::List(start)) => {
        flush_inline(&mut inline, &mut containers);
        containers.push(Container::List(List {
          ordered: start.is_some(),
          start: start.unwrap_or(1),
          items: Vec::new(),
        }));
      }
      Event::End(TagEnd::List(_)) => {
        flush_inline(&mut inline, &mut containers);
        let Some(Container::List(list)) = containers.pop() else {
          continue;
        };
        push_block(&mut containers, Block::List(list));
        blank_line(&mut containers);
      }
      Event::Start(Tag::Item) => {
        flush_inline(&mut inline, &mut containers);
        containers.push(Container::Item(ListItem {
          task: None,
          blocks: Vec::new(),
        }));
      }
      Event::End(TagEnd::Item) => {
        flush_inline(&mut inline, &mut containers);
        let Some(Container::Item(item)) = containers.pop() else {
          continue;
        };
        if let Some(Container::List(list)) = containers.last_mut() {
          list.items.push(item);
        }
      }
      Event::TaskListMarker(checked) => {
        if let Some(Container::Item(item)) = containers.last_mut() {
          item.task = Some(checked);
        }
      }
      Event::Start(Tag::Table(alignments)) => {
        flush_inline(&mut inline, &mut containers);
        containers.push(Container::Table(TableState {
          alignments: alignments.iter().copied().map(alignment).collect(),
          header: Vec::new(),
          rows: Vec::new(),
          current_row: Vec::new(),
        }));
      }
      Event::End(TagEnd::TableHead) => {
        if let Some(Container::Table(table)) = containers.last_mut() {
          table.header = std::mem::take(&mut table.current_row);
        }
      }
      Event::Start(Tag::TableCell) => inline.push(Vec::new()),
      Event::End(TagEnd::TableCell) => {
        let cell = inline.pop().unwrap_or_default();
        if let Some(Container::Table(table)) = containers.last_mut() {
          table.current_row.push(cell);
        }
      }
      Event::End(TagEnd::TableRow) => {
        if let Some(Container::Table(table)) = containers.last_mut() {
          let row = std::mem::take(&mut table.current_row);
          table.rows.push(row);
        }
      }
      Event::End(TagEnd::Table) => {
        let Some(Container::Table(table)) = containers.pop() else {
          continue;
        };
        push_block(
          &mut containers,
          Block::Table(Table {
            alignments: table.alignments,
            header: table.header,
            rows: table.rows,
          }),
        );
        blank_line(&mut containers);
      }
      Event::Text(text) => {
        if let Some((_, code)) = code.as_mut() {
          code.push_str(&text);
        } else if let Some(current) = inline.last_mut() {
          push_text(current, &text);
        }
      }
      Event::Code(text) => {
        if let Some(current) = inline.last_mut() {
          current.push(Inline::Code(text.to_string()));
        }
      }
      Event::SoftBreak => {
        if let Some(current) = inline.last_mut() {
          current.push(Inline::SoftBreak);
        }
      }
      Event::HardBreak => {
        if let Some(current) = inline.last_mut() {
          current.push(Inline::HardBreak);
        }
      }
      Event::Rule => {
        flush_inline(&mut inline, &mut containers);
        push_block(&mut containers, Block::Rule);
        blank_line(&mut containers);
      }
      _ => {}
    }
  }
  flush_inline(&mut inline, &mut containers);
  let Some(Container::Root(mut root)) = containers.pop() else {
    return Vec::new();
  };
  while matches!(root.last(), Some(Block::Blank)) {
    root.pop();
  }
  return root;
}

fn heading_level(level: HeadingLevel) -> u8 {
  return match level {
    HeadingLevel::H1 => 1,
    HeadingLevel::H2 => 2,
    HeadingLevel::H3 => 3,
    HeadingLevel::H4 => 4,
    HeadingLevel::H5 => 5,
    HeadingLevel::H6 => 6,
  };
}

fn alignment(align: MarkdownAlignment) -> Alignment {
  return match align {
    MarkdownAlignment::None => Alignment::None,
    MarkdownAlignment::Left => Alignment::Left,
    MarkdownAlignment::Center => Alignment::Center,
    MarkdownAlignment::Right => Alignment::Right,
  };
}

fn code_language(kind: CodeBlockKind<'_>) -> Option<String> {
  let CodeBlockKind::Fenced(info) = kind else {
    return None;
  };
  let language = info
    .split_whitespace()
    .next()
    .unwrap_or_default()
    .to_string();
  return (!language.is_empty()).then_some(language);
}

fn push_text(inlines: &mut Vec<Inline>, text: &str) {
  if text.is_empty() {
    return;
  }
  if let Some(Inline::Text(existing)) = inlines.last_mut() {
    existing.push_str(text);
    return;
  }
  inlines.push(Inline::Text(text.to_string()));
}

fn wrap_inline(inline: &mut Vec<Vec<Inline>>, wrap: fn(Vec<Inline>) -> Inline) {
  if inline.len() <= 1 {
    return;
  }
  let inner = inline.pop().unwrap_or_default();
  if let Some(parent) = inline.last_mut() {
    parent.push(wrap(inner));
  }
}

fn flush_inline(inline: &mut [Vec<Inline>], containers: &mut [Container]) {
  let inlines = std::mem::take(&mut inline[0]);
  push_paragraph(containers, inlines);
}

fn push_paragraph(containers: &mut [Container], inlines: Vec<Inline>) {
  if inlines.is_empty() {
    return;
  }
  push_block(containers, Block::Paragraph(inlines));
  blank_line(containers);
}

fn push_block(containers: &mut [Container], block: Block) {
  match containers.last_mut() {
    Some(Container::Root(blocks) | Container::Quote(blocks)) => {
      blocks.push(block);
    }
    Some(Container::Item(item)) => item.blocks.push(block),
    Some(Container::List(_) | Container::Table(_)) | None => {}
  }
}

fn blank_line(containers: &mut [Container]) {
  let Some(Container::Root(blocks) | Container::Quote(blocks)) =
    containers.last_mut()
  else {
    return;
  };
  if blocks
    .last()
    .is_some_and(|block| !matches!(block, Block::Blank))
  {
    blocks.push(Block::Blank);
  }
}

fn trim_blank_edges(blocks: &mut Vec<Block>) {
  while matches!(blocks.first(), Some(Block::Blank)) {
    blocks.remove(0);
  }
  while matches!(blocks.last(), Some(Block::Blank)) {
    blocks.pop();
  }
}
