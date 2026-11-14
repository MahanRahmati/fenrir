mod cli;

use anyhow::{Context, Result};
use clap::Parser;
use fenrir_cli::input::confirm_with;
use fenrir_core::config::Config;
use fenrir_core::config::secrets::Secrets;
use fenrir_core::database::DB;
use fenrir_core::engine::broker::{self, BrokerHandle};
use fenrir_core::engine::repo_actor::RepoActor;
use fenrir_core::logging::{set_print_enabled, setup_logging};
use fenrir_core::persona_manager::PersonaManager;
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
    Some(Commands::Tui) => run_tui(config).await,
    Some(Commands::Serve) => run_server(config).await,
    None => run_cli(config).await,
  };

  return match result {
    Ok(()) => {}
    Err(e) => {
      eprintln!("{e}");
      std::process::exit(1);
    }
  };
}

async fn run_tui(config: Config) -> Result<()> {
  let predict_enabled = config.general.predict_next_prompt;
  let default_persona = PersonaManager::default_for(&config).name().to_string();
  let broker = start_broker(&config).await?;
  return fenrir_tui::run(&broker, predict_enabled, &default_persona).await;
}

async fn run_server(config: Config) -> Result<()> {
  set_print_enabled(true);
  let broker = start_broker(&config).await?;
  return fenrir_server::run(&broker, config.server).await;
}

async fn run_cli(config: Config) -> Result<()> {
  set_print_enabled(true);
  let broker = start_broker(&config).await?;
  let server = fenrir_server::spawn(&broker, config.server).await?;
  let base_url = format!("http://{}", server.address);
  let result = fenrir_cli::run(&base_url).await;
  server.shutdown().await;
  return result;
}

async fn start_broker(config: &Config) -> Result<BrokerHandle> {
  let secrets = Secrets::load().await.context("Failed to load secrets")?;
  let db = DB::load().await.context("Failed to load database")?;
  let repository = SessionRepository::new(db);
  let repo = RepoActor::spawn(repository, config.clone());
  return Ok(broker::spawn(config.clone(), secrets, repo));
}
