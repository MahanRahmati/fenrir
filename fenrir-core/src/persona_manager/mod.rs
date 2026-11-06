use anyhow::{Result, bail};

mod compaction;
mod default;
mod judge;
mod persona;
mod prediction;
mod researcher;
mod reviewer;
mod titles;

pub use crate::persona_manager::compaction::COMPACTION_PERSONA;
pub use crate::persona_manager::default::DEFAULT_PERSONA;
pub use crate::persona_manager::judge::JUDGE_PERSONA;
pub use crate::persona_manager::persona::{Persona, ToolAccess};
pub use crate::persona_manager::prediction::PREDICTION_PERSONA;
use crate::persona_manager::researcher::RESEARCHER_PERSONA;
pub use crate::persona_manager::reviewer::REVIEWER_PERSONA;
pub use crate::persona_manager::titles::TITLES_PERSONA;

pub struct PersonaManager {
  current_persona: Persona,
}

impl PersonaManager {
  pub fn new() -> Self {
    return Self {
      current_persona: DEFAULT_PERSONA,
    };
  }

  pub fn current(&self) -> &Persona {
    return &self.current_persona;
  }

  pub fn list() -> Vec<Persona> {
    return [
      DEFAULT_PERSONA,
      RESEARCHER_PERSONA,
      REVIEWER_PERSONA,
      TITLES_PERSONA,
      COMPACTION_PERSONA,
      JUDGE_PERSONA,
    ]
    .into_iter()
    .filter(|p| p.is_selectable())
    .collect();
  }

  pub fn find(name: &str) -> Result<Persona> {
    let personas = Self::list();
    let Some(persona) = personas.into_iter().find(|p| p.name() == name) else {
      bail!("Unknown persona: {name}");
    };
    return Ok(persona);
  }

  pub fn set(&mut self, name: &str) -> Result<()> {
    self.current_persona = Self::find(name)?;
    return Ok(());
  }

  pub fn set_persona(&mut self, persona: Persona) {
    self.current_persona = persona;
  }
}

impl Default for PersonaManager {
  fn default() -> Self {
    return Self::new();
  }
}
