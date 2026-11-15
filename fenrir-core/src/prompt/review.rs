use crate::session_manager::session::Session;

const INSPECT: &str = "Use the git tool with `subcommand` `status` to list changed and untracked files, and `diff` for unstaged tracked changes. `diff` misses staged and untracked files, so read those directly. Do not modify any files.";

pub const CWD_REVIEW_PROMPT: &str =
  "Review the uncommitted changes in the current working directory.";

pub const SESSION_REVIEW_PROMPT: &str = "Review the changes made in this session. Use the conversation to identify what changed, then inspect the current files.";

pub fn build(session: &Session) -> String {
  let is_empty = session
    .messages
    .iter()
    .all(|message| message.role != "user");
  let subject = if is_empty {
    CWD_REVIEW_PROMPT
  } else {
    SESSION_REVIEW_PROMPT
  };
  return format!("{subject} {INSPECT}");
}
