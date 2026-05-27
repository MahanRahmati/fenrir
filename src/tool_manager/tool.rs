use std::collections::HashMap;

pub trait Tool {
  fn name(&self) -> &str;
  fn description(&self) -> &str;
  fn execute(&self, params: HashMap<String, String>) -> Result<String, String>;
}
