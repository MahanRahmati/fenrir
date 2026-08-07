use crate::persona_manager::Persona;

pub const DEFAULT_PERSONA: Persona = Persona::new(
  "Default",
  "General-purpose assistant",
  "You are Fenrir, a helpful AI assistant.",
  &[],
  true,
);
