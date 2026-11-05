pub const INSERT_MESSAGE: &str = include_str!("queries/insert_message.sql");
pub const LOAD_SESSION: &str = include_str!("queries/load_session.sql");
pub const LOAD_MESSAGES: &str = include_str!("queries/load_messages.sql");
pub const LIST_SESSIONS: &str = include_str!("queries/list_sessions.sql");
pub const INSERT_SESSION: &str = include_str!("queries/insert_session.sql");
pub const UPDATE_SESSION_TITLE: &str =
  include_str!("queries/update_session_title.sql");
pub const INSERT_SUMMARY: &str = include_str!("queries/insert_summary.sql");
pub const LOAD_SUMMARIES: &str = include_str!("queries/load_summaries.sql");
pub const INSERT_TOOL_APPROVAL: &str =
  include_str!("queries/insert_tool_approval.sql");
pub const RESOLVE_TOOL_APPROVAL: &str =
  include_str!("queries/resolve_tool_approval.sql");
pub const INSERT_QUEUED_MESSAGE: &str =
  include_str!("queries/insert_queued_message.sql");
pub const LOAD_QUEUED_MESSAGES: &str =
  include_str!("queries/load_queued_messages.sql");
pub const LOAD_NEXT_QUEUED_MESSAGE: &str =
  include_str!("queries/load_next_queued_message.sql");
pub const CLAIM_QUEUED_MESSAGE: &str =
  include_str!("queries/claim_queued_message.sql");
pub const DELETE_QUEUED_MESSAGE: &str =
  include_str!("queries/delete_queued_message.sql");
pub const RELEASE_QUEUED_MESSAGE: &str =
  include_str!("queries/release_queued_message.sql");
pub const RESET_PROCESSING_QUEUED_MESSAGES: &str =
  include_str!("queries/reset_processing_queued_messages.sql");
pub const CLEAR_QUEUED_MESSAGES: &str =
  include_str!("queries/clear_queued_messages.sql");
