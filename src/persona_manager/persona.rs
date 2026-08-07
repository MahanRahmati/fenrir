#[derive(Debug, Clone)]
pub struct Persona {
  name: &'static str,
  description: &'static str,
  identity: &'static str,
  instructions: &'static [&'static str],
  selectable: bool,
}

impl Persona {
  pub const fn new(
    name: &'static str,
    description: &'static str,
    identity: &'static str,
    instructions: &'static [&'static str],
    selectable: bool,
  ) -> Self {
    return Self {
      name,
      description,
      identity,
      instructions,
      selectable,
    };
  }

  pub fn name(&self) -> &str {
    return self.name;
  }

  pub fn description(&self) -> &str {
    return self.description;
  }

  pub fn identity(&self) -> &str {
    return self.identity;
  }

  pub fn instructions(&self) -> &[&'static str] {
    return self.instructions;
  }

  pub fn is_selectable(&self) -> bool {
    return self.selectable;
  }
}
