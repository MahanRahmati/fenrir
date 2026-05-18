use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fenrir")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = concat!("Fenrir v", env!("CARGO_PKG_VERSION")))]
pub struct Cli {
  #[command(subcommand)]
  pub command: Option<Commands>,

  /// Use verbose output
  #[arg(short, long, default_value_t = false, global = true)]
  pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
  /// Reset configuration to default values
  ResetConfig {
    /// Skip confirmation prompt
    #[arg(short, long, default_value_t = false)]
    yes: bool,
  },
}
