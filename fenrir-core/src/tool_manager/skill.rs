use std::fmt::Write;

use anyhow::{Result, bail};

use crate::skill_manager::SkillRegistry;
use crate::tool_manager::helpers::get_param;
use crate::tool_manager::tool::{Tool, ToolContext, ToolParams};

pub const SKILL_TOOL_NAME: &str = "skill";

pub struct SkillTool {
  description: String,
  names: Vec<String>,
}

impl SkillTool {
  pub fn new(skills: &SkillRegistry) -> Self {
    let mut description = String::from(
      "Load a skill's instructions by name. Call this before starting a task that matches a skill's description. Skills may include supporting files in their directory.",
    );
    description.push_str("\n\nAvailable skills:");
    let mut names = Vec::new();
    for skill in skills.model_invocable() {
      let _ = write!(
        description,
        "\n- {} ({}): {}",
        skill.name,
        skill.scope.name(),
        skill.description
      );
      names.push(skill.name.to_string());
    }
    return Self { description, names };
  }
}

#[async_trait::async_trait]
impl Tool for SkillTool {
  fn name(&self) -> &str {
    return SKILL_TOOL_NAME;
  }

  fn description(&self) -> &str {
    return &self.description;
  }

  fn parameters(&self) -> serde_json::Value {
    return serde_json::json!({
      "type": "object",
      "properties": {
        "name": {
          "type": "string",
          "enum": &self.names,
          "description": "Name of the skill to load",
        },
      },
      "required": ["name"],
    });
  }

  fn is_read_only(&self) -> bool {
    return true;
  }

  async fn execute(
    &self,
    params: ToolParams,
    context: &mut ToolContext<'_>,
  ) -> Result<String> {
    let name = get_param::<String>(&params, SKILL_TOOL_NAME, "name")?;
    let Some(skill) = context.skills.find(&name) else {
      bail!("Unknown skill '{name}'");
    };
    return Ok(format!(
      "Skill: {}\nScope: {}\nDirectory: {}\n\n{}",
      skill.name,
      skill.scope.name(),
      skill.directory.display(),
      skill.content
    ));
  }
}
