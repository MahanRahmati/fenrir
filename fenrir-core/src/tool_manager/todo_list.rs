use anyhow::{Context, Result, bail};
use uuid::Uuid;

use crate::files::operations::{
  create_directory_all, read_to_string, write_file,
};
use crate::tool_manager::helpers::{get_optional_param, get_param};
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};
use crate::utils::xdg;

pub const TODO_LIST_TOOL_NAME: &str = "todo_list";

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct TodoItem {
  item: String,
  status: TodoStatus,
}

#[derive(
  Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
enum TodoStatus {
  Pending,
  InProgress,
  Completed,
}

impl TodoStatus {
  fn marker(&self) -> &'static str {
    return match self {
      TodoStatus::Pending => " ",
      TodoStatus::InProgress => ">",
      TodoStatus::Completed => "x",
    };
  }
}

#[derive(Clone, Debug, PartialEq)]
struct TodoList {
  title: String,
  items: Vec<TodoItem>,
}

pub struct TodoListTool;

impl TodoListTool {
  async fn todos_path(session_id: Uuid) -> Result<String> {
    let data_home =
      xdg::data_home().context("Cannot determine the fenrir data directory")?;
    let dir = data_home.join("todos");
    create_directory_all(dir.to_string_lossy().as_ref())
      .await
      .context(format!("Cannot create directory '{}'", dir.display()))?;
    return Ok(
      dir
        .join(format!("{session_id}.md"))
        .to_string_lossy()
        .to_string(),
    );
  }

  fn parse_todos(raw: &str) -> Result<Vec<TodoItem>> {
    let mut value: serde_json::Value = serde_json::from_str(raw)
      .context("todos must be a JSON array of {item, status} objects")?;
    if let serde_json::Value::String(inner) = value {
      value = serde_json::from_str(&inner)
        .context("todos must be a JSON array of {item, status} objects")?;
    }
    return serde_json::from_value(value)
      .context("todos must be a JSON array of {item, status} objects");
  }

  fn parse_markdown(content: &str) -> TodoList {
    let mut list = TodoList {
      title: String::new(),
      items: Vec::new(),
    };
    for line in content.lines() {
      if let Some(item) = line.strip_prefix("- [x] ") {
        list.items.push(TodoItem {
          item: item.to_string(),
          status: TodoStatus::Completed,
        });
      } else if let Some(item) = line.strip_prefix("- [>] ") {
        list.items.push(TodoItem {
          item: item.to_string(),
          status: TodoStatus::InProgress,
        });
      } else if let Some(item) = line.strip_prefix("- [ ] ") {
        list.items.push(TodoItem {
          item: item.to_string(),
          status: TodoStatus::Pending,
        });
      } else if list.title.is_empty() {
        list.title = line.trim().to_string();
      }
    }
    return list;
  }

  fn validate(title: &str, items: &[TodoItem]) -> Result<TodoList> {
    if items.is_empty() {
      bail!("todos must contain at least one item");
    }
    for item in items {
      if item.item.trim().is_empty() {
        bail!("todo item cannot be empty");
      }
    }
    let in_progress = items
      .iter()
      .filter(|t| t.status == TodoStatus::InProgress)
      .count();
    if in_progress > 1 {
      bail!("At most one todo can be in_progress, got {in_progress}");
    }
    return Ok(TodoList {
      title: title.trim().to_string(),
      items: items.to_vec(),
    });
  }

  fn render(list: &TodoList) -> String {
    let done = list
      .items
      .iter()
      .filter(|t| t.status == TodoStatus::Completed)
      .count();
    let label = if list.title.is_empty() {
      "Todo list"
    } else {
      &list.title
    };
    let lines: Vec<String> = list
      .items
      .iter()
      .map(|t| format!("- [{}] {}", t.status.marker(), t.item))
      .collect();
    return format!(
      "{label} ({done}/{} done):\n{}",
      list.items.len(),
      lines.join("\n")
    );
  }

  fn file_content(list: &TodoList) -> String {
    let lines: Vec<String> = list
      .items
      .iter()
      .map(|t| format!("- [{}] {}", t.status.marker(), t.item))
      .collect();
    if list.title.is_empty() {
      return lines.join("\n");
    }
    return format!("{}\n{}", list.title, lines.join("\n"));
  }

  async fn save(path: &str, list: &TodoList) -> Result<()> {
    let content = Self::file_content(list);
    return write_file(path, &content)
      .await
      .context(format!("Cannot write todo file '{path}'"));
  }

  async fn show(session_id: Uuid) -> Result<String> {
    let path = Self::todos_path(session_id).await?;
    let Some(content) = read_to_string(&path).await.ok() else {
      return Ok("No todo list yet.".to_string());
    };
    let list = Self::parse_markdown(&content);
    if list.items.is_empty() {
      return Ok("No todo list yet.".to_string());
    }
    return Ok(Self::render(&list));
  }
}

#[async_trait::async_trait]
impl Tool for TodoListTool {
  fn name(&self) -> &str {
    return TODO_LIST_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return "Create and update a todo list to track progress during multi-step work. The list is stored per session and persists across turns. Pass the full list as a JSON array of {item, status} objects on every call; it replaces the previous list. Include a short title if one fits. Pass the single word 'show' as todos to display the current list. At most one item may be in_progress.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "title": {
          "type": "string",
          "description": "Short title for the list (optional)",
        },
        "todos": {
          "type": "string",
          "description": "The full todo list as a JSON array of {item, status} objects (item: string, status: one of pending, in_progress, completed), replacing the previous list, or the single word 'show' to display the current list",
        },
      },
      "required": ["todos"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(
    &self,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let raw = get_param::<String>(&params, TODO_LIST_TOOL_NAME, "todos")?;
    if raw.trim() == "show" {
      return Self::show(context.session_id).await;
    }
    let title =
      get_optional_param::<String>(&params, TODO_LIST_TOOL_NAME, "title")
        .unwrap_or_default();
    let items = Self::parse_todos(&raw)?;
    let list = Self::validate(&title, &items)?;
    let path = Self::todos_path(context.session_id).await?;
    let existing = read_to_string(&path).await.ok();
    if let Some(content) = existing
      && Self::parse_markdown(&content) == list
    {
      return Ok("Todo list unchanged since last update.".to_string());
    }
    Self::save(&path, &list).await?;
    return Ok(Self::render(&list));
  }
}
