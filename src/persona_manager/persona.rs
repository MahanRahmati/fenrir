#[derive(Debug, Clone, Copy)]
pub enum ToolAccess {
  All,
  Only(&'static [&'static str]),
}

impl ToolAccess {
  pub fn allows(&self, name: &str) -> bool {
    return match self {
      ToolAccess::All => true,
      ToolAccess::Only(tools) => tools.contains(&name),
    };
  }
}

#[derive(Debug, Clone)]
pub struct Persona {
  name: &'static str,
  description: &'static str,
  identity: &'static str,
  instructions: &'static [&'static str],
  allowed_tools: ToolAccess,
  selectable: bool,
}

impl Persona {
  pub const fn new(
    name: &'static str,
    description: &'static str,
    identity: &'static str,
    instructions: &'static [&'static str],
    allowed_tools: ToolAccess,
    selectable: bool,
  ) -> Self {
    return Self {
      name,
      description,
      identity,
      instructions,
      allowed_tools,
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

  pub fn allowed_tools(&self) -> ToolAccess {
    return self.allowed_tools;
  }

  pub fn is_selectable(&self) -> bool {
    return self.selectable;
  }
}
