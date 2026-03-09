use std::{env, fs, path::Path};

use diesel::{
  connection::SimpleConnection,
  sqlite::SqliteConnection,
};
use generated::{CargoCreate, CargoDb, CargoDbFilter, CargoUpdate, NamespaceCreate, NamespaceDb};
use prismar::{PrismaClient, PrismaReadManyInput, Provider, SqlBackend, StringFilter, connection_pool, with_connection};

#[path = "../generated/mod.rs"]
mod generated;

const FIXTURE_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[ntex::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let database_url = env::var("DATABASE_URL")
    .unwrap_or_else(|_| format!("sqlite://{FIXTURE_DIR}/dev.db"));

  reset_sqlite_file(&database_url)?;
  let migration_sql = load_migration_sql()?;

  let pool = connection_pool::<SqliteConnection>(database_url.clone())?;
  let client = PrismaClient::new(Provider::Sqlite, database_url.clone())?;

  with_connection(pool.clone(), move |conn| {
    conn.batch_execute("PRAGMA foreign_keys = ON;")?;
    for sql in &migration_sql {
      conn.batch_execute(&sql)?;
    }
    Ok(())
  })
  .await?;

  with_connection(pool.clone(), |conn| {
    conn.batch_execute("PRAGMA foreign_keys = ON;")?;
    Ok(())
  })
  .await?;

  client
    .create(NamespaceCreate {
      name: Some("system".to_owned()),
    })
    .await?;

  client
    .create(CargoCreate {
      key: Some("cargo.system.api".to_owned()),
      created_at: Some(
        chrono::NaiveDate::from_ymd_opt(2026, 3, 9)
          .unwrap()
          .and_hms_opt(12, 0, 0)
          .unwrap(),
      ),
      name: Some("api".to_owned()),
      spec_key: None,
      status_key: Some("running".to_owned()),
      namespace_name: Some("system".to_owned()),
    })
    .await?;

  let payload = fs::read_to_string(Path::new(FIXTURE_DIR).join("query.json"))?;
  let input: PrismaReadManyInput = serde_json::from_str(&payload)?;
  let filter = input
    .to_args()
    .map_err(std::io::Error::other)?
    .r#where
    .ok_or_else(|| std::io::Error::other("missing where filter"))?;

  let rendered = filter.render(SqlBackend::Sqlite);
  assert!(rendered.sql.contains("key = ?"));

  let cargos = client.find_many::<CargoDb>(None).await?;
  let namespaces = client.find_many::<NamespaceDb>(None).await?;
  let filtered = client
    .find_many::<CargoDb>(Some(
      CargoDbFilter::new()
        .key(StringFilter::Equals("cargo.system.api".to_owned()))
        .into(),
    ))
    .await?;
  let cargo_id = "cargo.system.api".to_owned();
  let cargo = client
    .find_by_id::<CargoDb>(&cargo_id)
    .await?
    .ok_or_else(|| std::io::Error::other("cargo not found"))?;

  assert_eq!(namespaces.len(), 1);
  assert_eq!(cargos.len(), 1);
  assert_eq!(filtered.len(), 1);
  assert_eq!(cargo.namespace_name, "system");
  assert_eq!(cargo.name, "api");
  assert!(!cargo.spec_key.is_empty());

  client
    .update_by_id::<CargoUpdate>(
      &cargo_id,
      CargoUpdate {
        status_key: Some("updated".to_owned()),
        ..Default::default()
      },
    )
    .await?;

  let updated = client
    .find_by_id::<CargoDb>(&cargo_id)
    .await?
    .ok_or_else(|| std::io::Error::other("updated cargo not found"))?;
  assert_eq!(updated.status_key, "updated");

  client.delete_by_id::<CargoDb>(&cargo_id).await?;
  assert!(client.find_many::<CargoDb>(None).await?.is_empty());

  println!("E2E fixture passed: migrations applied and generated models queried successfully.");
  Ok(())
}

fn reset_sqlite_file(database_url: &str) -> Result<(), Box<dyn std::error::Error>> {
  let normalized = database_url
    .strip_prefix("sqlite://")
    .unwrap_or(database_url);
  let normalized = normalized.strip_prefix("sqlite:").unwrap_or(normalized);

  if normalized == ":memory:" {
    return Ok(());
  }

  let path = Path::new(normalized);
  if let Some(parent) = path.parent() {
    if !parent.as_os_str().is_empty() {
      fs::create_dir_all(parent)?;
    }
  }
  if path.exists() {
    fs::remove_file(path)?;
  }
  Ok(())
}

fn load_migration_sql() -> Result<Vec<String>, Box<dyn std::error::Error>> {
  let mut files = fs::read_dir(Path::new(FIXTURE_DIR).join("migrations"))?
    .filter_map(Result::ok)
    .map(|entry| entry.path().join("migration.sql"))
    .filter(|path| path.exists())
    .collect::<Vec<_>>();
  files.sort();

  let mut out = Vec::new();
  for file in files {
    out.push(fs::read_to_string(file)?);
  }
  Ok(out)
}