mod migrations;
mod queries;

use anyhow::{Context, Result};
use chrono::Utc;
use turso::{Database, Rows};

use crate::database::migrations::list_migrations;
use crate::database::queries::*;
use crate::utils::xdg;

const DEFAULT_DB_NAME: &str = "database.db";

pub struct DB {
  db: Database,
}

impl DB {
  pub async fn load() -> Result<Self> {
    let db_path = xdg::place_data_file(DEFAULT_DB_NAME)
      .context("Failed to initialize database")?;
    let db = turso::Builder::new_local(db_path.to_string_lossy().as_ref())
      .experimental_multiprocess_wal(true)
      .build()
      .await
      .context("Failed to initialize database")?;
    let db_instance = DB { db };
    db_instance.run_migrations().await?;
    return Ok(db_instance);
  }

  async fn run_migrations(&self) -> Result<()> {
    let conn = self.connect()?;

    let applied: Vec<String> =
      match conn.query(LIST_APPLIED_MIGRATIONS, ()).await {
        Ok(mut rows) => {
          let mut result = Vec::new();
          while let Ok(Some(row)) = rows.next().await {
            let name: String =
              row.get(0).context("Failed to read applied migration")?;
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
        .context(format!("Failed to apply migration '{}'", migration.name))?;
      let record = conn
        .execute(
          RECORD_MIGRATION,
          (migration.name.as_str(), Utc::now().timestamp()),
        )
        .await;
      if let Err(e) = record {
        if e.to_string().contains("UNIQUE") {
          continue;
        }
        return Err(e)
          .context(format!("Failed to record migration '{}'", migration.name));
      }
    }
    return Ok(());
  }

  pub async fn execute(
    &self,
    sql: &str,
    params: impl turso::IntoParams,
  ) -> Result<()> {
    let conn = self.connect()?;
    conn
      .execute(sql, params)
      .await
      .context("Failed to execute query")?;
    return Ok(());
  }

  pub async fn query(
    &self,
    sql: &str,
    params: impl turso::IntoParams,
  ) -> Result<Rows> {
    let conn = self.connect()?;
    return conn.query(sql, params).await.context("Failed to run query");
  }

  fn connect(&self) -> Result<turso::Connection> {
    let conn = self.db.connect().context("Failed to connect to database")?;
    conn
      .busy_timeout(std::time::Duration::from_millis(500))
      .context("Failed to set database busy timeout")?;
    return Ok(conn);
  }
}
