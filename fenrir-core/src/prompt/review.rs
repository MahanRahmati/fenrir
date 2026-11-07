use crate::session_manager::session::Session;

pub const CWD_REVIEW_PROMPT: &str = "Review the uncommitted changes in the current working directory. Use the git tool with `status` to list changed files and `diff` to inspect unstaged changes, and read the changed files you need. Report bugs, risky behavior changes, and clear deviations from existing patterns. Do not modify any files.";
pub const SESSION_REVIEW_PROMPT: &str = "Review the changes made in this session. Use the conversation to identify what changed, inspect the current state of those files, and use the git tool with `status` to list changed files and `diff` to inspect unstaged changes. Report bugs, risky behavior changes, and clear deviations from existing patterns. Do not modify any files.";

pub fn build(session: &Session) -> String {
  let is_empty = session
    .messages
    .iter()
    .all(|message| message.role != "user");
  return if is_empty {
    CWD_REVIEW_PROMPT.to_string()
  } else {
    SESSION_REVIEW_PROMPT.to_string()
  };
}
