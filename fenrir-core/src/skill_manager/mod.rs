use std::fs::{read_dir, read_to_string};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::utils::xdg;
use crate::vlog;

const SKILL_FILE: &str = "SKILL.md";
const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;
const PROJECT_FENRIR_SKILLS: &str = ".fenrir/skills";
const PROJECT_AGENTS_SKILLS: &str = ".agents/skills";
const GLOBAL_FENRIR_SKILLS: &str = "skills";
const GLOBAL_AGENTS_SKILLS: &str = ".agents/skills";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
  Project,
  Global,
}

impl SkillScope {
  pub fn name(&self) -> &'static str {
    return match self {
      Self::Project => "project",
      Self::Global => "global",
    };
  }
}

#[derive(Debug, Clone)]
pub struct Skill {
  pub name: String,
  pub description: String,
  pub directory: PathBuf,
  pub content: String,
  pub scope: SkillScope,
  pub model_invocable: bool,
}

#[derive(Debug, Default)]
pub struct SkillRegistry {
  skills: Vec<Skill>,
}

impl SkillRegistry {
  pub fn load(cwd: &Path) -> Self {
    return Self::load_from_roots(&skill_roots(cwd));
  }

  fn load_from_roots(roots: &[SkillRoot]) -> Self {
    let mut skills: Vec<Skill> = Vec::new();
    for root in roots {
      let Ok(entries) = read_dir(&root.path) else {
        continue;
      };
      for entry in entries.flatten() {
        let directory = entry.path();
        if !directory.is_dir() {
          continue;
        }
        let path = directory.join(SKILL_FILE);
        if !path.is_file() {
          continue;
        }
        match load_skill(&path, &directory, root.scope) {
          Ok(skill) => {
            if skills.iter().any(|existing| existing.name == skill.name) {
              continue;
            }
            skills.push(skill);
          }
          Err(error) => {
            vlog!("Skipping skill '{}': {error}", path.display());
          }
        }
      }
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    return Self { skills };
  }

  pub fn skills(&self) -> &[Skill] {
    return &self.skills;
  }

  pub fn model_invocable(&self) -> impl Iterator<Item = &Skill> {
    return self.skills.iter().filter(|skill| skill.model_invocable);
  }

  pub fn has_model_invocable(&self) -> bool {
    return self.skills.iter().any(|skill| skill.model_invocable);
  }

  pub fn find(&self, name: &str) -> Option<&Skill> {
    return self.skills.iter().find(|skill| skill.name == name);
  }
}

struct SkillRoot {
  path: PathBuf,
  scope: SkillScope,
}

fn skill_roots(cwd: &Path) -> Vec<SkillRoot> {
  let mut roots = vec![
    SkillRoot {
      path: cwd.join(PROJECT_FENRIR_SKILLS),
      scope: SkillScope::Project,
    },
    SkillRoot {
      path: cwd.join(PROJECT_AGENTS_SKILLS),
      scope: SkillScope::Project,
    },
  ];
  if let Ok(config_home) = xdg::config_home() {
    roots.push(SkillRoot {
      path: config_home.join(GLOBAL_FENRIR_SKILLS),
      scope: SkillScope::Global,
    });
  }
  if let Some(home) = xdg::home_dir() {
    roots.push(SkillRoot {
      path: home.join(GLOBAL_AGENTS_SKILLS),
      scope: SkillScope::Global,
    });
  }
  return roots;
}

struct SkillFrontmatter {
  name: String,
  description: String,
  model_invocable: bool,
}

fn load_skill(
  path: &Path,
  directory: &Path,
  scope: SkillScope,
) -> Result<Skill> {
  let raw = read_to_string(path)
    .context(format!("Cannot read '{}'", path.display()))?;
  let (frontmatter, content) = split_frontmatter(&raw)?;
  let frontmatter = parse_frontmatter(&frontmatter)?;
  let name = frontmatter.name;
  let description = frontmatter.description;
  if !is_valid_name(&name) {
    bail!("Invalid skill name '{name}'");
  }
  if name.chars().count() > MAX_NAME_LENGTH {
    bail!("Skill name '{name}' exceeds {MAX_NAME_LENGTH} characters");
  }
  if description.is_empty() {
    bail!("Skill '{name}' has an empty description");
  }
  if description.chars().count() > MAX_DESCRIPTION_LENGTH {
    bail!(
      "Skill '{name}' description exceeds {MAX_DESCRIPTION_LENGTH} characters"
    );
  }
  let directory_name = directory
    .file_name()
    .map(|name| name.to_string_lossy().to_string())
    .unwrap_or_default();
  if name != directory_name {
    bail!("Skill name '{name}' does not match directory '{directory_name}'");
  }
  return Ok(Skill {
    name,
    description,
    directory: directory.to_path_buf(),
    content,
    scope,
    model_invocable: frontmatter.model_invocable,
  });
}

fn parse_frontmatter(frontmatter: &str) -> Result<SkillFrontmatter> {
  let mut name: Option<String> = None;
  let mut description: Option<String> = None;
  let mut model_invocable = true;
  for line in frontmatter.lines() {
    if name.is_none()
      && let Some(value) = line.strip_prefix("name: ")
    {
      name = Some(value.to_string());
      continue;
    }
    if description.is_none()
      && let Some(value) = line.strip_prefix("description: ")
    {
      description = Some(value.to_string());
      continue;
    }
    if line == "disable-model-invocation: false" {
      model_invocable = false;
    }
  }
  let Some(name) = name else {
    bail!("Missing 'name' in frontmatter");
  };
  let Some(description) = description else {
    bail!("Missing 'description' in frontmatter");
  };
  return Ok(SkillFrontmatter {
    name,
    description,
    model_invocable,
  });
}

fn split_frontmatter(raw: &str) -> Result<(String, String)> {
  let mut lines = raw.lines();
  let Some(first) = lines.next() else {
    bail!("Missing YAML frontmatter");
  };
  if first.trim_end() != "---" {
    bail!("Missing YAML frontmatter");
  }
  let mut frontmatter = String::new();
  let mut content = String::new();
  let mut in_frontmatter = true;
  for line in lines {
    if in_frontmatter {
      if line.trim_end() == "---" {
        in_frontmatter = false;
        continue;
      }
      frontmatter.push_str(line);
      frontmatter.push('\n');
    } else {
      content.push_str(line);
      content.push('\n');
    }
  }
  if in_frontmatter {
    bail!("Unterminated YAML frontmatter");
  }
  return Ok((frontmatter, content.trim().to_string()));
}

fn is_valid_name(name: &str) -> bool {
  if name.is_empty() || name.starts_with('-') || name.ends_with('-') {
    return false;
  }
  return name
    .chars()
    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
}
