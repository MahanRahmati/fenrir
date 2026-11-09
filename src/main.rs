mod cli;

use clap::Parser;
use fenrir_cli::input::confirm_with;
use fenrir_core::config::Config;
use fenrir_core::config::secrets::Secrets;
use fenrir_core::database::DB;
use fenrir_core::engine::broker;
use fenrir_core::engine::repo_actor::RepoActor;
use fenrir_core::logging::{set_print_enabled, setup_logging};
use fenrir_core::session_manager::repository::SessionRepository;

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() {
  let cli = Cli::parse();

  match setup_logging(cli.verbose) {
    Ok(()) => (),
    Err(e) => {
      eprintln!("Log Error: {e}");
      std::process::exit(1);
    }
  };

  let config = match Config::load().await {
    Ok(config) => config,
    Err(e) => {
      eprintln!("Configuration Error: {e}");
      std::process::exit(1);
    }
  };

  let secrets = match Secrets::load().await {
    Ok(secrets) => secrets,
    Err(e) => {
      eprintln!("Failed to load secrets: {e}");
      std::process::exit(1);
    }
  };

  let db = match DB::load().await {
    Ok(db) => db,
    Err(e) => {
      eprintln!("Failed to load database: {e}");
      std::process::exit(1);
    }
  };

  let result = match cli.command {
    Some(Commands::ResetConfig { yes }) => {
      if !yes
        && !confirm_with(
          "Are you sure you want to reset the configuration to default \
           values? (y/n) ",
        )
      {
        println!("Reset cancelled.");
        return;
      }
      match Config::reset_to_defaults().await {
        Ok(_) => {
          println!("Configuration has been reset to default values.");
          return;
        }
        Err(e) => {
          eprintln!("Failed to reset configuration: {e}");
          std::process::exit(1);
        }
      }
    }
    Some(Commands::Tui) => {
      let predict_enabled = config.general.predict_next_prompt;
      let repository = SessionRepository::new(db);
      let repo = RepoActor::spawn(repository, config.clone());
      let broker = broker::spawn(config, secrets, repo);
      fenrir_tui::run(&broker, predict_enabled).await
    }
    Some(Commands::Serve) => {
      set_print_enabled(true);
      let repository = SessionRepository::new(db);
      let repo = RepoActor::spawn(repository, config.clone());
      let broker = broker::spawn(config.clone(), secrets, repo);
      fenrir_server::run(&broker, config.server).await
    }
    None => {
      set_print_enabled(true);
      let repository = SessionRepository::new(db);
      let repo = RepoActor::spawn(repository, config.clone());
      let broker = broker::spawn(config, secrets, repo);
      fenrir_cli::run(&broker).await
    }
  };

  return match result {
    Ok(()) => {}
    Err(e) => {
      eprintln!("{e}");
      std::process::exit(1);
    }
  };
}
