use anyhow::{Result, bail};

mod compaction;
mod default;
mod persona;
mod researcher;
mod reviewer;
mod titles;

pub use crate::persona_manager::compaction::COMPACTION_PERSONA;
use crate::persona_manager::default::DEFAULT_PERSONA;
pub use crate::persona_manager::persona::{Persona, ToolAccess};
use crate::persona_manager::researcher::RESEARCHER_PERSONA;
use crate::persona_manager::reviewer::REVIEWER_PERSONA;
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

  pub fn list(&self) -> Vec<Persona> {
    return [
      DEFAULT_PERSONA,
      RESEARCHER_PERSONA,
      REVIEWER_PERSONA,
      TITLES_PERSONA,
      COMPACTION_PERSONA,
    ]
    .into_iter()
    .filter(|p| p.is_selectable())
    .collect();
  }

  pub fn set(&mut self, name: &str) -> Result<()> {
    let personas = self.list();
    let Some(persona) = personas.iter().find(|p| p.name() == name) else {
      bail!("Unknown persona: {name}");
    };
    self.current_persona = persona.clone();
    return Ok(());
  }

  pub fn set_persona(&mut self, persona: Persona) {
    self.current_persona = persona;
  }

  pub fn reset(&mut self) {
    self.current_persona = DEFAULT_PERSONA;
  }
}

impl Default for PersonaManager {
  fn default() -> Self {
    return Self::new();
  }
}
