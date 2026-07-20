#[derive(Debug, Clone)]
pub struct Model {
  pub name: String,
  pub alias: Option<String>,
}

pub trait Provider {
  fn name(&self) -> &str;
  fn active_model(&self) -> Option<&Model>;
  fn list_models(&self) -> Vec<Model>;
}
