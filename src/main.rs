mod cli;

use std::sync::Arc;

use clap::Parser;
use fenrir_cli::input::confirm_with;
use fenrir_core::app::App;
use fenrir_core::config::Config;
use fenrir_core::database::DB;
use fenrir_core::logging::{set_print_enabled, setup_logging};
use tokio::sync::Mutex;

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

  let db = match DB::load().await {
    Ok(db) => db,
    Err(e) => {
      eprintln!("Failed to load database: {e}");
      std::process::exit(1);
    }
  };

  let app = Arc::new(Mutex::new(App::new(config, db)));

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
    Some(Commands::Tui) => fenrir_tui::run(&app).await,
    None => {
      set_print_enabled(true);
      fenrir_cli::run(&app).await
    }
  };

  return match result {
    Ok(output) => println!("{output}"),
    Err(e) => {
      eprintln!("{e}");
      std::process::exit(1);
    }
  };
}
