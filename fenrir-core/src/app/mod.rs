mod approval;
pub mod command;
mod event;

pub use approval::{ApprovalChannel, ApprovalDecision};
pub use event::{AppEvent, EventSink};

use crate::agent_manager::AgentManager;
use crate::config::Config;
use crate::database::DB;
use crate::shutdown::Shutdown;

pub struct App {
  pub agent_manager: AgentManager,
  pub shutdown: Shutdown,
}

impl App {
  pub fn new(config: Config, db: DB) -> Self {
    let agent_manager = AgentManager::new(config, db);
    let shutdown = Shutdown::new();
    return App {
      agent_manager,
      shutdown,
    };
  }
}
