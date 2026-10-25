use anyhow::{Context, Result, bail};

use crate::tool_manager::helpers::{get_optional_param, get_param};
use crate::tool_manager::tool::{Tool, ToolParams};
use crate::utils::process;

pub struct MusicTool;

#[async_trait::async_trait]
impl Tool for MusicTool {
  fn name(&self) -> &str {
    return "music";
  }

  fn description(&self) -> &str {
    return "Control music playback through the MPD daemon using mpc. Use 'play_song' with a query to find and play a song by artist, album, or title. Use 'play', 'pause', 'next', 'previous' for playback control. Use 'volume' to set or adjust volume. Use 'add' to add a file to the queue, 'clear' to empty the queue. Use 'status' to see current playback info. Random and repeat are always on.";
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "description": "Action: play (start/resume playback), pause, next, previous, volume (set or adjust volume), add (add file to queue), clear (empty queue), status (show current playback info), play_song (find and play a song by artist, album, or title)"
        },
        "value": {
          "type": "string",
          "description": "For volume: number 0-100"
        },
        "adjust": {
          "type": "string",
          "description": "For volume: relative adjustment like +10 or -5"
        },
        "position": {
          "type": "string",
          "description": "For play: queue position number"
        },
        "file": {
          "type": "string",
          "description": "For add: path to music file"
        },
        "query": {
          "type": "string",
          "description": "For play_song: search term"
        }
      },
      "required": ["action"],
    });
  }

  fn is_read_only(&self) -> bool {
    return false;
  }

  async fn execute(&self, params: ToolParams) -> Result<String> {
    let action = get_param::<String>(&params, "music", "action")?;

    if action != "status" {
      let _ = ensure_modes().await;
    }

    if action == "play_song" {
      let query = get_param::<String>(&params, "music", "query")?;

      let mut found = false;
      for search_type in &["title", "artist", "album"] {
        let search_args = ["search", search_type, &query];
        let search_output = process::run("mpc", &search_args)
          .await
          .context("Failed to search song")?;

        let Some(first_line) = search_output.stdout.trim().lines().next()
        else {
          continue;
        };
        let file_path = first_line.trim();
        if file_path.is_empty() {
          continue;
        }

        let add_output = process::run("mpc", &["add", file_path])
          .await
          .context("Failed to add song")?;
        if !add_output.exit_code.is_success() {
          continue;
        }

        let playlist =
          process::run("mpc", &["playlist", "-f", "%position%:%file%"])
            .await
            .context("Failed to read queue")?;

        let position = playlist.stdout.lines().find_map(|line| {
          let (pos, path) = line.split_once(':')?;
          if path == file_path {
            Some(pos.to_string())
          } else {
            None
          }
        });

        let Some(position) = position else {
          continue;
        };

        let output = process::run("mpc", &["play", &position])
          .await
          .context("Failed to play")?;

        check_mpc_error(&output)?;

        found = true;
        break;
      }

      if !found {
        bail!("No songs found matching '{query}'");
      }

      return Ok("Song playing".to_string());
    }

    let args = build_mpc_args(&action, &params)?;
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let output = process::run("mpc", &args_refs)
      .await
      .context("Failed to execute mpc")?;

    check_mpc_error(&output)?;

    let stdout = output.stdout.trim().to_string();

    return Ok(if stdout.is_empty() {
      "Command executed successfully".to_string()
    } else {
      stdout
    });
  }
}

async fn ensure_modes() -> Result<()> {
  let _ = process::run("mpc", &["random", "on"]).await;
  let _ = process::run("mpc", &["repeat", "on"]).await;
  return Ok(());
}

fn build_mpc_args(action: &str, params: &ToolParams) -> Result<Vec<String>> {
  let mut args = vec![action.to_string()];

  match action {
    "play" => {
      if let Some(pos) = params.get("position").cloned() {
        args.push(pos);
      }
    }
    "pause" | "next" | "clear" => {}
    "previous" => {
      args = vec!["prev".to_string()];
    }
    "volume" => {
      let value = get_optional_param::<String>(params, "music", "value");
      let adjust = get_optional_param::<String>(params, "music", "adjust");
      match (value, adjust) {
        (Some(v), _) => args.push(v),
        (_, Some(a)) => args.push(a),
        (None, None) => {}
      }
    }

    "add" => {
      if let Some(file) = get_optional_param::<String>(params, "music", "file")
      {
        args.push(file);
      }
    }
    "status" => {}
    _ => {
      bail!(
        "Invalid action '{action}'. Valid actions: play, pause, next, previous, volume, add, clear, status, play_song"
      );
    }
  }

  return Ok(args);
}

fn check_mpc_error(output: &process::Output) -> Result<()> {
  if output.exit_code.is_success() {
    return Ok(());
  }
  let stderr = output.stderr.trim();
  let reason = if stderr.contains("Connection refused")
    || stderr.contains("Connection closed")
  {
    "MPD is not running.".to_string()
  } else {
    format!("mpc error: {stderr}")
  };
  bail!("{reason}");
}
