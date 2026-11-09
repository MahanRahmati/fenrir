use tokio::sync::mpsc::Sender;

use crate::config::Config;
use crate::config::secrets::Secrets;
use crate::engine::protocol::SessionEvent;
use crate::engine::repo_actor::RepoHandle;
use crate::prompt::{prediction, title};
use crate::session_manager::session::Session;

pub fn spawn_prediction(
  session: &Session,
  config: &Config,
  secrets: &Secrets,
  events: Sender<SessionEvent>,
) {
  prediction::spawn(session, config, secrets, events);
}

pub fn spawn_title(
  session: &mut Session,
  config: &Config,
  secrets: &Secrets,
  repo: &RepoHandle,
  events: &Sender<SessionEvent>,
) {
  title::spawn_if_needed(session, config, secrets, repo, events);
}
