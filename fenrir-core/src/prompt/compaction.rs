use crate::config::Config;
use crate::persona_manager::COMPACTION_PERSONA;
use crate::prompt::{PromptKind, PromptSpec, system};
use crate::provider_manager::Model;
use crate::session_manager::{Message, Summary};

pub fn build(
  config: &Config,
  cwd: &str,
  model: &Model,
  messages: Vec<Message>,
  last_summary: Option<Summary>,
) -> PromptSpec {
  let mut system_prompt = system::build(config, &COMPACTION_PERSONA, cwd);
  if let Some(ref summary) = last_summary {
    system_prompt.push_str("\n\nPrevious summary:\n");
    system_prompt.push_str(&summary.content);
  }

  let max_chars = config.compaction.tool_result_max_chars;
  let serialized: Vec<Message> = messages
    .iter()
    .filter(|message| message.role != "system")
    .filter_map(|message| {
      let content = message.serialize_for_compaction(max_chars);
      if content.is_empty() {
        return None;
      }
      Some(Message {
        id: message.id,
        msg_type: message.msg_type.clone(),
        role: message.role.clone(),
        content,
        tool_call_id: message.tool_call_id.clone(),
        tool_calls: message.tool_calls.clone(),
        timestamp: message.timestamp,
      })
    })
    .collect();

  let mut prompt_messages = Vec::new();
  if let Some(ref summary) = last_summary {
    let first_user_message =
      serialized.iter().find(|message| message.role == "user");
    if let Some(message) = first_user_message {
      prompt_messages.push(message.clone());
    }
    for message in &serialized {
      if message.id > summary.covers_message_id {
        prompt_messages.push(message.clone());
      }
    }
  } else {
    prompt_messages = serialized;
  }

  return PromptSpec {
    kind: PromptKind::Compaction,
    model: model.clone(),
    system: system_prompt,
    messages: prompt_messages,
    tools: Vec::new(),
    stream: false,
    emit_tokens: false,
  };
}
