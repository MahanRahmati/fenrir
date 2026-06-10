pub mod errors;

use turso::{Database, Rows};

use crate::config::DEFAULT_DIRECTORY;
use crate::database::errors::{DatabaseError, DatabaseResult};

const DEFAULT_DB_NAME: &str = "database.db";

pub struct DB {
  db: Database,
}

impl DB {
  pub async fn load() -> DatabaseResult<Self> {
    let xdg_dirs = xdg::BaseDirectories::with_prefix(DEFAULT_DIRECTORY);
    let db_path = xdg_dirs
      .place_data_file(DEFAULT_DB_NAME)
      .map_err(|e| DatabaseError::Initialization(e.to_string()))?;
    let db = turso::Builder::new_local(db_path.to_string_lossy().as_ref())
      .build()
      .await
      .map_err(|e| DatabaseError::Initialization(e.to_string()))?;
    return Ok(DB { db });
  }

  pub async fn execute(
    &self,
    sql: &str,
    params: impl turso::IntoParams,
  ) -> DatabaseResult<()> {
    let conn = self.connect()?;
    conn
      .execute(sql, params)
      .await
      .map_err(|e| DatabaseError::Query(e.to_string()))?;
    return Ok(());
  }

  pub async fn query(
    &self,
    sql: &str,
    params: impl turso::IntoParams,
  ) -> DatabaseResult<Rows> {
    let conn = self.connect()?;
    return conn
      .query(sql, params)
      .await
      .map_err(|e| DatabaseError::Query(e.to_string()));
  }

  fn connect(&self) -> DatabaseResult<turso::Connection> {
    let conn = self
      .db
      .connect()
      .map_err(|e| DatabaseError::Connection(e.to_string()))?;
    return Ok(conn);
  }
}
