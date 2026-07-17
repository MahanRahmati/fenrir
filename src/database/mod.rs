mod errors;
mod migrations;
mod queries;

use chrono::Utc;
use turso::{Database, Rows};

use crate::config::DEFAULT_DIRECTORY;
use crate::database::errors::{DatabaseError, DatabaseResult};
use crate::database::migrations::list_migrations;
use crate::database::queries::*;

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
    let db_instance = DB { db };
    db_instance.run_migrations().await?;
    return Ok(db_instance);
  }

  async fn run_migrations(&self) -> DatabaseResult<()> {
    let conn = self.connect()?;

    let applied: Vec<String> =
      match conn.query(LIST_APPLIED_MIGRATIONS, ()).await {
        Ok(mut rows) => {
          let mut result = Vec::new();
          while let Ok(Some(row)) = rows.next().await {
            let name: String = row
              .get(0)
              .map_err(|e| DatabaseError::Migration(e.to_string()))?;
            result.push(name);
          }
          result
        }
        Err(_) => Vec::new(),
      };

    for migration in list_migrations() {
      if applied.contains(&migration.name) {
        continue;
      }
      conn
        .execute(&migration.sql, ())
        .await
        .map_err(|e| DatabaseError::Migration(e.to_string()))?;
      conn
        .execute(
          RECORD_MIGRATION,
          (migration.name.as_str(), Utc::now().timestamp()),
        )
        .await
        .map_err(|e| DatabaseError::Migration(e.to_string()))?;
    }
    return Ok(());
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
