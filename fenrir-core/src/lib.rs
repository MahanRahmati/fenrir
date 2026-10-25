//! Core harness for Fenrir.

pub mod agent_manager;
pub mod app;
mod capability;
pub mod config;
pub mod database;
pub mod files;
mod hook_manager;
pub mod logging;
mod mentions;
mod network;
pub mod persona_manager;
mod prompt;
pub mod provider_manager;
pub mod session_manager;
mod shutdown;
pub mod slash_commands;
pub mod tool_manager;
pub mod utils;
mod workflow_manager;
