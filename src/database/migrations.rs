pub struct Migration {
  pub name: String,
  pub sql: String,
}

pub fn list_migrations() -> Vec<Migration> {
  return vec![
    Migration {
      name: "000_schema_migrations".into(),
      sql: include_str!("../migrations/000_schema_migrations.sql").into(),
    },
    Migration {
      name: "001_create_sessions".into(),
      sql: include_str!("../migrations/001_create_sessions.sql").into(),
    },
    Migration {
      name: "002_create_messages".into(),
      sql: include_str!("../migrations/002_create_messages.sql").into(),
    },
  ];
}
