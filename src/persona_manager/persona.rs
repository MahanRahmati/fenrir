#[derive(Debug, Clone)]
pub struct Persona {
  name: &'static str,
  description: &'static str,
  identity: &'static str,
  instructions: &'static [&'static str],
}

impl Persona {
  pub const fn new(
    name: &'static str,
    description: &'static str,
    identity: &'static str,
    instructions: &'static [&'static str],
  ) -> Self {
    return Self {
      name,
      description,
      identity,
      instructions,
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
}
