use crate::persona_manager::{Persona, ToolAccess};

pub const DEFAULT_PERSONA: Persona = Persona::new(
  "Default",
  "General-purpose assistant",
  "You are Fenrir, a helpful AI assistant.",
  &[],
  ToolAccess::All,
  true,
);
