mod app;
mod cli;
mod config;
mod database;
mod files;
mod input;
mod logging;
mod network;
mod prompt;
mod provider_manager;
mod session_manager;
mod slash_commands;
mod tool_manager;
mod utils;

use clap::Parser;

use crate::app::App;
use crate::cli::{Cli, Commands};
use crate::config::Config;
use crate::database::DB;
use crate::logging::set_verbose;

#[tokio::main]
async fn main() {
  let cli = Cli::parse();

  set_verbose(cli.verbose);

  let config = match Config::load().await {
    Ok(config) => config,
    Err(e) => {
      eprintln!("Configuration Error: {}", e);
      std::process::exit(1);
    }
  };

  let db = match DB::load().await {
    Ok(db) => db,
    Err(e) => {
      eprintln!("Failed to load database: {}", e);
      std::process::exit(1);
    }
  };

  let mut app = App::new(config, db);

  let result = match cli.command {
    Some(Commands::ResetConfig { yes }) => {
      match Config::reset_to_defaults(yes).await {
        Ok(_) => {
          println!("Configuration has been reset to default values.");
          return;
        }
        Err(e) => {
          eprintln!("Failed to reset configuration: {}", e);
          std::process::exit(1);
        }
      }
    }
    None => app.run().await,
  };

  return match result {
    Ok(output) => println!("{}", output),
    Err(e) => {
      eprintln!("{}", e);
      std::process::exit(1);
    }
  };
}
