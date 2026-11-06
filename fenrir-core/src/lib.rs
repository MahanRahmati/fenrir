//! Core harness for Fenrir.

pub mod app;
mod capability;
mod command;
pub mod config;
pub mod database;
pub mod engine;
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
