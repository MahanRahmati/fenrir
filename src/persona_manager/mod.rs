mod default;
mod persona;
mod reviewer;

use crate::persona_manager::default::DEFAULT_PERSONA;
pub use crate::persona_manager::persona::Persona;
use crate::persona_manager::reviewer::REVIEWER_PERSONA;

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

  pub fn list(&self) -> &'static [Persona] {
    return &[DEFAULT_PERSONA, REVIEWER_PERSONA];
  }

  pub fn set(&mut self, name: &str) -> Result<(), String> {
    let persona = self
      .list()
      .iter()
      .find(|p| p.name() == name)
      .ok_or_else(|| format!("Unknown persona: {name}"))?;
    self.current_persona = persona.clone();
    return Ok(());
  }

  pub fn reset(&mut self) {
    self.current_persona = DEFAULT_PERSONA;
  }
}
