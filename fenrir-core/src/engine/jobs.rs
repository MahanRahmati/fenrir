use tokio::sync::mpsc::Sender;

use crate::config::Config;
use crate::engine::protocol::SessionEvent;
use crate::engine::repo_actor::RepoHandle;
use crate::prompt::{prediction, title};
use crate::session_manager::session::Session;

pub fn spawn_prediction(
  session: &Session,
  config: &Config,
  events: Sender<SessionEvent>,
) {
  prediction::spawn(session, config, events);
}

pub fn spawn_title(
  session: &mut Session,
  config: &Config,
  repo: &RepoHandle,
  events: &Sender<SessionEvent>,
) {
  title::spawn_if_needed(session, config, repo, events);
}
