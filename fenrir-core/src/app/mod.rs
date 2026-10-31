mod approval;
pub mod command;
mod event;
mod harness;

pub use approval::{ApprovalChannel, ApprovalDecision};
pub use event::{AppEvent, EventSink};

use crate::config::Config;
use crate::database::DB;
use crate::session_manager::SessionManager;
use crate::shutdown::Shutdown;

pub struct App {
  pub sessions: SessionManager,
  pub shutdown: Shutdown,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let sessions = SessionManager::new(config, db);
    let shutdown = Shutdown::new();
    return App { sessions, shutdown };
  }
}
