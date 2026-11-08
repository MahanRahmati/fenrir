use uuid::Uuid;

pub enum Control {
  Cancel { turn_id: Option<Uuid> },
}
