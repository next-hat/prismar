use std::{env, fs, path::Path};

use diesel::{
  connection::SimpleConnection,
  sqlite::SqliteConnection,
};
use generated::{
  CargoCreate, CargoDb, CargoDbFilter, CargoUpdate, NamespaceCreate,
  NamespaceDb, NamespaceDbFilter,
};
use prismar::{PrismaClient, PrismaReadManyInput, Provider, SqlBackend, StringFilter, connection_pool, with_connection};

#[path = "../generated/mod.rs"]
mod generated;

const FIXTURE_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Debug)]
struct NamespaceWithCargoes {
  namespace: NamespaceDb,
  cargoes: Vec<CargoDb>,
}

#[derive(Debug)]
struct CargoWithNamespace {
  cargo: CargoDb,
  namespace: Option<NamespaceDb>,
}

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

  let namespace = client
    .create(NamespaceCreate {
      name: Some("system".to_owned()),
    })
    .await?;
  assert_eq!(namespace.name, "system");

  let created = client
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
  assert_eq!(created.key, "cargo.system.api");

  let batch = client
    .create_many(
      vec![CargoCreate {
        key: Some("cargo.system.worker".to_owned()),
        created_at: Some(
          chrono::NaiveDate::from_ymd_opt(2026, 3, 9)
            .unwrap()
            .and_hms_opt(12, 5, 0)
            .unwrap(),
        ),
        name: Some("worker".to_owned()),
        spec_key: None,
        status_key: Some("running".to_owned()),
        namespace_name: Some("system".to_owned()),
      }],
      None,
    )
    .await?;
  assert_eq!(batch.count, 1);

  let payload = fs::read_to_string(Path::new(FIXTURE_DIR).join("query.json"))?;
  let input: PrismaReadManyInput = serde_json::from_str(&payload)?;
  let filter = input
    .to_args()
    .map_err(std::io::Error::other)?
    .r#where
    .ok_or_else(|| std::io::Error::other("missing where filter"))?;

  let rendered = filter.render(SqlBackend::Sqlite);
  assert!(rendered.sql.contains("key = ?"));
  assert!(rendered.sql.contains("EXISTS"));

  let cargos = client.find_many::<CargoDb>(None).await?;
  let namespaces = client.find_many::<NamespaceDb>(None).await?;
  let json_filtered = client.find_many::<CargoDb>(Some(filter)).await?;
  let filtered = client
    .find_many::<CargoDb>(Some(
      CargoDbFilter::new()
        .key(StringFilter::Equals("cargo.system.api".to_owned()))
        .into(),
    ))
    .await?;
  let relation_filtered_cargos = client
    .find_many::<CargoDb>(Some(
      CargoDbFilter::new().namespace_is(
        NamespaceDbFilter::new().name(StringFilter::Equals("system".to_owned())),
      ).into(),
    ))
    .await?;
  let relation_filtered_namespaces = client
    .find_many::<NamespaceDb>(Some(
      NamespaceDbFilter::new().cargos_some(
        CargoDbFilter::new().name(StringFilter::Equals("api".to_owned())),
      ).into(),
    ))
    .await?;
  let unique = client
    .find_unique::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
    )
    .await?
    .ok_or_else(|| std::io::Error::other("unique cargo not found"))?;
  let first = client
    .find_first::<CargoDb>(Some(
      CargoDbFilter::new()
        .status_key(StringFilter::Equals("running".to_owned()))
        .into(),
    ))
    .await?
    .ok_or_else(|| std::io::Error::other("first cargo not found"))?;
  let total = client.count::<CargoDb>(None).await?;
  let cargo_id = "cargo.system.api".to_owned();
  let cargo = client
    .find_by_id::<CargoDb>(&cargo_id)
    .await?
    .ok_or_else(|| std::io::Error::other("cargo not found"))?;
  let namespace_with_cargoes = NamespaceWithCargoes {
    namespace: namespace.clone(),
    cargoes: namespace.cargos(&client, None).await?,
  };
  let cargo_with_namespace = CargoWithNamespace {
    cargo: cargo.clone(),
    namespace: cargo.namespace(&client).await?,
  };

  assert_eq!(namespaces.len(), 1);
  assert_eq!(cargos.len(), 2);
  assert_eq!(json_filtered.len(), 2);
  assert_eq!(filtered.len(), 1);
  assert_eq!(relation_filtered_cargos.len(), 2);
  assert_eq!(relation_filtered_namespaces.len(), 1);
  assert_eq!(total, 2);
  assert_eq!(unique.key, "cargo.system.api");
  assert_eq!(first.status_key, "running");
  assert_eq!(namespace_with_cargoes.namespace.name, "system");
  assert_eq!(namespace_with_cargoes.cargoes.len(), 2);
  assert_eq!(cargo_with_namespace.cargo.key, "cargo.system.api");
  assert_eq!(cargo_with_namespace.namespace.as_ref().map(|item| item.name.as_str()), Some("system"));
  assert_eq!(cargo.namespace_name, "system");
  assert_eq!(cargo.name, "api");
  assert!(!cargo.spec_key.is_empty());

  let updated = client
    .update::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals(cargo_id.clone())),
      CargoUpdate {
        status_key: Some("updated".to_owned()),
        ..Default::default()
      },
    )
    .await?;
  assert_eq!(updated.status_key, "updated");

  let updated_many = client
    .update_many::<CargoDb>(
      Some(
        CargoDbFilter::new()
          .status_key(StringFilter::Equals("running".to_owned()))
          .into(),
      ),
      CargoUpdate {
        status_key: Some("queued".to_owned()),
        ..Default::default()
      },
    )
    .await?;
  assert_eq!(updated_many.count, 1);

  let updated_many_rows = client
    .update_many_and_return::<CargoDb>(
      Some(
        CargoDbFilter::new()
          .status_key(StringFilter::Equals("queued".to_owned()))
          .into(),
      ),
      CargoUpdate {
        status_key: Some("drained".to_owned()),
        ..Default::default()
      },
    )
    .await?;
  assert_eq!(updated_many_rows.len(), 1);
  assert_eq!(updated_many_rows[0].status_key, "drained");

  let upserted = client
    .upsert::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals("cargo.system.upsert".to_owned())),
      CargoCreate {
        key: Some("cargo.system.upsert".to_owned()),
        created_at: Some(
          chrono::NaiveDate::from_ymd_opt(2026, 3, 9)
            .unwrap()
            .and_hms_opt(12, 10, 0)
            .unwrap(),
        ),
        name: Some("upsert".to_owned()),
        spec_key: None,
        status_key: Some("created".to_owned()),
        namespace_name: Some("system".to_owned()),
      },
      CargoUpdate {
        status_key: Some("updated".to_owned()),
        ..Default::default()
      },
    )
    .await?;
  assert_eq!(upserted.key, "cargo.system.upsert");

  let deleted = client
    .delete::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals(cargo_id.clone())),
    )
    .await?;
  assert_eq!(deleted.key, cargo_id);

  let deleted_many = client
    .delete_many::<CargoDb>(Some(
      CargoDbFilter::new()
        .namespace_name(StringFilter::Equals("system".to_owned()))
        .into(),
    ))
    .await?;
  assert_eq!(deleted_many.count, 2);
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
