mod generate;

use std::{
  collections::{BTreeMap, BTreeSet},
  env, fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "mysql")]
use diesel::mysql::MysqlConnection;
#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
use diesel::sql_types::{Nullable, Text};
use diesel::{
  Connection as DieselConnection, QueryableByName, RunQueryDsl, sql_query,
};
use prismar_migrate::{
  backend_from_provider, default_migration_name, diff_schema_sql,
};
use prismar_schema::{
  Datasource, Field, FieldAttribute, FieldType, Model, Schema, parse_schema,
};
use rusqlite::Connection;

const DIESEL_MIGRATIONS_TABLE: &str = "__diesel_schema_migrations";

#[derive(Debug, Parser)]
#[command(name = "prismar")]
#[command(about = "Prismar schema and migration CLI")]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
  Validate {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
  },
  Generate {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
  },
  Migrate {
    #[command(subcommand)]
    command: MigrateCommands,
  },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProviderArg {
  Sqlite,
  Postgresql,
  Mysql,
}

impl ProviderArg {
  fn as_provider(self) -> &'static str {
    match self {
      Self::Sqlite => "sqlite",
      Self::Postgresql => "postgresql",
      Self::Mysql => "mysql",
    }
  }
}

#[derive(Debug, Subcommand)]
enum MigrateCommands {
  Diff {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
    #[arg(long)]
    from: Option<PathBuf>,
    #[arg(long, value_enum)]
    provider: Option<ProviderArg>,
  },
  Dev {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
    #[arg(long)]
    name: Option<String>,
    #[arg(long, default_value = "prismar/migrations")]
    migrations_dir: PathBuf,
    #[arg(long, value_enum)]
    provider: Option<ProviderArg>,
  },
  Deploy {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
    #[arg(long, default_value = "prismar/migrations")]
    migrations_dir: PathBuf,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long, value_enum)]
    provider: Option<ProviderArg>,
  },
  Status {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
    #[arg(long, default_value = "prismar/migrations")]
    migrations_dir: PathBuf,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long, value_enum)]
    provider: Option<ProviderArg>,
  },
  Drift {
    #[arg(long, default_value = "schema.prisma")]
    schema: PathBuf,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long, value_enum)]
    provider: Option<ProviderArg>,
  },
}

fn main() -> Result<()> {
  let cli = Cli::parse();

  match cli.command {
    Commands::Validate { schema } => {
      let parsed = load_schema(&schema)?;
      println!(
        "schema is valid (provider={}, models={})",
        display_provider(&parsed),
        parsed.models.len()
      );
    }
    Commands::Generate { schema, output } => {
      generate::run_generate(&schema, output)?;
    }
    Commands::Migrate { command } => match command {
      MigrateCommands::Diff {
        schema,
        from,
        provider,
      } => {
        let current = load_schema(&schema)?;
        let previous = match from {
          Some(path) => load_schema(&path)?,
          None => Schema {
            datasource: current.datasource.clone(),
            generators: current.generators.clone(),
            models: Vec::new(),
          },
        };
        let backend = determine_backend(&current, provider);
        let statements = diff_schema_sql(&previous, &current, backend);
        if statements.is_empty() {
          println!("No schema changes detected.");
        } else {
          println!("{}", statements.join("\n"));
        }
      }
      MigrateCommands::Dev {
        schema,
        name,
        migrations_dir,
        provider,
      } => {
        let current = load_schema(&schema)?;
        let backend = determine_backend(&current, provider);
        let snapshot_path = migrations_dir.join(".schema.prisma");

        let previous = if snapshot_path.exists() {
          load_schema(&snapshot_path)?
        } else {
          Schema {
            datasource: current.datasource.clone(),
            generators: current.generators.clone(),
            models: Vec::new(),
          }
        };

        let statements = diff_schema_sql(&previous, &current, backend);
        if statements.is_empty() {
          println!("No migration required.");
          return Ok(());
        }

        let version = default_migration_name();
        let migration_name =
          sanitize_name(name.unwrap_or_else(|| "migration".to_owned()));
        let folder =
          migrations_dir.join(format!("{}_{}", version, migration_name));
        fs::create_dir_all(&folder).with_context(|| {
          format!("failed to create migration folder {}", folder.display())
        })?;

        let migration_file = folder.join("migration.sql");
        fs::write(&migration_file, statements.join("\n") + "\n").with_context(
          || {
            format!(
              "failed to write migration file {}",
              migration_file.display()
            )
          },
        )?;

        if !migrations_dir.exists() {
          fs::create_dir_all(&migrations_dir).with_context(|| {
            format!(
              "failed to create migrations directory {}",
              migrations_dir.display()
            )
          })?;
        }

        fs::write(&snapshot_path, fs::read_to_string(&schema)?).with_context(
          || {
            format!(
              "failed to write schema snapshot {}",
              snapshot_path.display()
            )
          },
        )?;

        println!("Created migration {}", migration_file.display());
      }
      MigrateCommands::Status {
        schema,
        migrations_dir,
        database_url,
        provider,
      } => {
        if !migrations_dir.exists() {
          println!(
            "No migrations directory found at {}",
            migrations_dir.display()
          );
          return Ok(());
        }

        let parsed = load_schema(&schema)?;
        let backend = determine_backend(&parsed, provider);
        let local = load_migration_files(&migrations_dir)?;

        println!("Local migrations: {}", local.len());
        if local.is_empty() {
          println!("No migrations found.");
          return Ok(());
        }

        if let Some(url) =
          resolve_database_url(parsed.datasource.as_ref(), database_url)
        {
          match backend {
            prismar_migrate::SqlBackend::Sqlite => {
              #[cfg(not(feature = "sqlite"))]
              {
                return Err(anyhow::anyhow!(
                  "sqlite support is disabled; recompile with --features sqlite"
                ));
              }
              #[cfg(feature = "sqlite")]
              {
                let conn = sqlite_connect(&url)?;
                ensure_migrations_table(&conn)?;
                let applied = load_applied_map(&conn)?;
                print_status(local, applied);
              }
            }
            prismar_migrate::SqlBackend::Postgres
            | prismar_migrate::SqlBackend::MySql => {
              let applied = load_applied_map_diesel(&url, backend)?;
              print_status(local, applied);
            }
          }
        } else {
          println!("No database url available. Showing local migrations only:");
          for migration in &local {
            println!("- {}", migration.folder);
          }
        }
      }
      MigrateCommands::Deploy {
        schema,
        migrations_dir,
        database_url,
        provider,
      } => {
        let parsed = load_schema(&schema)?;
        let backend = determine_backend(&parsed, provider);
        let url =
          resolve_database_url(parsed.datasource.as_ref(), database_url)
            .ok_or_else(|| {
              anyhow::anyhow!("database url is required for deploy")
            })?;

        let migrations = load_migration_files(&migrations_dir)?;
        if migrations.is_empty() {
          println!("No migrations to deploy.");
          return Ok(());
        }

        match backend {
          prismar_migrate::SqlBackend::Sqlite => {
            #[cfg(not(feature = "sqlite"))]
            {
              return Err(anyhow::anyhow!(
                "sqlite support is disabled; recompile with --features sqlite"
              ));
            }
            #[cfg(feature = "sqlite")]
            {
              let conn = sqlite_connect(&url)?;
              ensure_migrations_table(&conn)?;
              let applied = load_applied_map(&conn)?;

              let mut applied_count = 0usize;
              for migration in migrations {
                if applied.contains_key(&migration.version) {
                  continue;
                }

                conn.execute_batch("BEGIN TRANSACTION;")?;
                let apply_result = conn.execute_batch(&migration.sql);
                match apply_result {
                  Ok(_) => {
                    let insert_sql = format!(
                      "INSERT INTO {DIESEL_MIGRATIONS_TABLE} (version, run_on) VALUES (?1, ?2)"
                    );
                    conn.execute(
                      &insert_sql,
                      (migration.version.clone(), Utc::now().to_rfc3339()),
                    )?;
                    conn.execute_batch("COMMIT;")?;
                    println!("Applied {}", migration.folder);
                    applied_count += 1;
                  }
                  Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    return Err(anyhow::anyhow!(
                      "failed to apply migration {}: {}",
                      migration.folder,
                      err
                    ));
                  }
                }
              }

              println!(
                "Deploy complete. Applied {} migration(s).",
                applied_count
              );
            }
          }
          prismar_migrate::SqlBackend::Postgres
          | prismar_migrate::SqlBackend::MySql => {
            let applied_count = deploy_diesel(&url, backend, migrations)?;
            println!(
              "Deploy complete. Applied {} migration(s).",
              applied_count
            );
          }
        }
      }
      MigrateCommands::Drift {
        schema,
        database_url,
        provider,
      } => {
        let parsed = load_schema(&schema)?;
        let backend = determine_backend(&parsed, provider);
        let url =
          resolve_database_url(parsed.datasource.as_ref(), database_url)
            .ok_or_else(|| {
              anyhow::anyhow!("database url is required for drift checks")
            })?;

        match backend {
          prismar_migrate::SqlBackend::Sqlite => {
            #[cfg(not(feature = "sqlite"))]
            {
              return Err(anyhow::anyhow!(
                "sqlite support is disabled; recompile with --features sqlite"
              ));
            }
            #[cfg(feature = "sqlite")]
            {
              let conn = sqlite_connect(&url)?;
              let live_schema = introspect_sqlite_schema(&conn)?;
              let statements = diff_schema_sql(&live_schema, &parsed, backend);
              if statements.is_empty() {
                println!("No drift detected.");
              } else {
                println!("Drift detected. SQL needed to reconcile:");
                println!("{}", statements.join("\n"));
              }
            }
          }
          prismar_migrate::SqlBackend::Postgres
          | prismar_migrate::SqlBackend::MySql => {
            let live_schema = introspect_sql_schema(&url, backend)?;
            let statements = diff_schema_sql(&live_schema, &parsed, backend);
            if statements.is_empty() {
              println!("No drift detected.");
            } else {
              println!("Drift detected. SQL needed to reconcile:");
              println!("{}", statements.join("\n"));
            }
          }
        }
      }
    },
  }

  Ok(())
}

fn load_schema(path: &Path) -> Result<Schema> {
  if !path.exists() {
    bail!("schema file does not exist: {}", path.display());
  }

  let content = fs::read_to_string(path)
    .with_context(|| format!("failed to read schema {}", path.display()))?;
  parse_schema(&content).map_err(|err| anyhow::anyhow!(err))
}

fn provider(schema: &Schema) -> &str {
  schema
    .datasource
    .as_ref()
    .map(|source| source.provider.as_str())
    .unwrap_or("postgresql")
}

fn display_provider(schema: &Schema) -> &str {
  schema
    .datasource
    .as_ref()
    .map(|source| source.provider.as_str())
    .unwrap_or("dynamic")
}

fn determine_backend(
  schema: &Schema,
  provider_override: Option<ProviderArg>,
) -> prismar_migrate::SqlBackend {
  if let Some(provider_override) = provider_override {
    backend_from_provider(provider_override.as_provider())
  } else {
    backend_from_provider(provider(schema))
  }
}

fn resolve_database_url(
  datasource: Option<&Datasource>,
  cli_database_url: Option<String>,
) -> Option<String> {
  if let Some(url) = cli_database_url {
    return Some(url);
  }

  if let Ok(url) = env::var("DATABASE_URL")
    && !url.trim().is_empty()
  {
    return Some(url);
  }

  let raw = datasource?.url.as_ref()?.trim().to_owned();
  if raw.starts_with("env(") {
    let env_name = raw
      .trim_start_matches("env(")
      .trim_end_matches(')')
      .trim()
      .trim_matches('"')
      .to_owned();
    env::var(env_name).ok()
  } else if raw.is_empty() {
    None
  } else {
    Some(raw)
  }
}

fn sqlite_connect(database_url: &str) -> Result<Connection> {
  let normalized = database_url
    .strip_prefix("sqlite://")
    .unwrap_or(database_url);
  let normalized = normalized.strip_prefix("sqlite:").unwrap_or(normalized);
  Connection::open(normalized).with_context(|| {
    format!("failed to open sqlite database at {}", database_url)
  })
}

fn ensure_migrations_table(conn: &Connection) -> Result<()> {
  let sql = format!(
    "CREATE TABLE IF NOT EXISTS {DIESEL_MIGRATIONS_TABLE} (
      version TEXT PRIMARY KEY,
      run_on TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );"
  );
  conn
    .execute_batch(&sql)
    .context("failed to ensure diesel migrations table")
}

fn load_applied_map(conn: &Connection) -> Result<BTreeMap<String, String>> {
  let query = format!("SELECT version FROM {DIESEL_MIGRATIONS_TABLE}");
  let mut stmt = conn.prepare(&query)?;
  let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
  let mut out = BTreeMap::new();
  for row in rows {
    out.insert(row?, String::new());
  }
  Ok(out)
}

fn print_status(local: Vec<MigrationFile>, applied: BTreeMap<String, String>) {
  let mut pending = Vec::new();

  for migration in &local {
    if !applied.contains_key(&migration.version) {
      pending.push(migration.folder.clone());
    }
  }

  let local_versions = local
    .iter()
    .map(|migration| migration.version.clone())
    .collect::<BTreeSet<_>>();
  let missing_local = applied
    .keys()
    .filter(|version| !local_versions.contains(*version))
    .cloned()
    .collect::<Vec<_>>();

  println!("Applied in DB: {}", applied.len());
  println!("Pending: {}", pending.len());
  println!("Missing locally: {}", missing_local.len());

  if !pending.is_empty() {
    println!("Pending migrations:");
    for name in pending {
      println!("- {}", name);
    }
  }

  if !missing_local.is_empty() {
    println!("Applied in DB but missing locally:");
    for version in missing_local {
      println!("- {}", version);
    }
  }
}

fn load_applied_map_diesel(
  database_url: &str,
  backend: prismar_migrate::SqlBackend,
) -> Result<BTreeMap<String, String>> {
  match backend {
    prismar_migrate::SqlBackend::Postgres => {
      #[cfg(feature = "postgres")]
      {
        let mut conn = connect_postgres(database_url)?;
        ensure_migrations_table_pg(&mut conn)?;
        load_applied_map_pg(&mut conn)
      }
      #[cfg(not(feature = "postgres"))]
      {
        Err(anyhow::anyhow!(
          "postgres support is disabled; recompile with --features postgres"
        ))
      }
    }
    prismar_migrate::SqlBackend::MySql => {
      #[cfg(feature = "mysql")]
      {
        let mut conn = connect_mysql(database_url)?;
        ensure_migrations_table_mysql(&mut conn)?;
        load_applied_map_mysql(&mut conn)
      }
      #[cfg(not(feature = "mysql"))]
      {
        Err(anyhow::anyhow!(
          "mysql support is disabled; recompile with --features mysql"
        ))
      }
    }
    prismar_migrate::SqlBackend::Sqlite => {
      Err(anyhow::anyhow!("diesel applied map is not used for sqlite"))
    }
  }
}

fn deploy_diesel(
  database_url: &str,
  backend: prismar_migrate::SqlBackend,
  migrations: Vec<MigrationFile>,
) -> Result<usize> {
  match backend {
    prismar_migrate::SqlBackend::Postgres => {
      #[cfg(feature = "postgres")]
      {
        let mut conn = connect_postgres(database_url)?;
        ensure_migrations_table_pg(&mut conn)?;
        let applied = load_applied_map_pg(&mut conn)?;
        deploy_with_pg(&mut conn, migrations, applied)
      }
      #[cfg(not(feature = "postgres"))]
      {
        Err(anyhow::anyhow!(
          "postgres support is disabled; recompile with --features postgres"
        ))
      }
    }
    prismar_migrate::SqlBackend::MySql => {
      #[cfg(feature = "mysql")]
      {
        let mut conn = connect_mysql(database_url)?;
        ensure_migrations_table_mysql(&mut conn)?;
        let applied = load_applied_map_mysql(&mut conn)?;
        deploy_with_mysql(&mut conn, migrations, applied)
      }
      #[cfg(not(feature = "mysql"))]
      {
        Err(anyhow::anyhow!(
          "mysql support is disabled; recompile with --features mysql"
        ))
      }
    }
    prismar_migrate::SqlBackend::Sqlite => {
      Err(anyhow::anyhow!("diesel deploy is not used for sqlite"))
    }
  }
}

fn split_sql_statements(sql: &str) -> Vec<&str> {
  sql
    .split(';')
    .map(str::trim)
    .filter(|stmt| !stmt.is_empty())
    .collect()
}

#[cfg(feature = "postgres")]
fn ensure_migrations_table_pg(conn: &mut PgConnection) -> Result<()> {
  let sql = format!(
    "CREATE TABLE IF NOT EXISTS {DIESEL_MIGRATIONS_TABLE} (
      version VARCHAR(50) PRIMARY KEY,
      run_on TIMESTAMP NOT NULL DEFAULT NOW()
    )"
  );
  sql_query(sql)
    .execute(conn)
    .context("failed to ensure diesel migrations table")?;
  Ok(())
}

#[cfg(feature = "mysql")]
fn ensure_migrations_table_mysql(conn: &mut MysqlConnection) -> Result<()> {
  let sql = format!(
    "CREATE TABLE IF NOT EXISTS {DIESEL_MIGRATIONS_TABLE} (
      version VARCHAR(50) PRIMARY KEY,
      run_on DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
    )"
  );
  sql_query(sql)
    .execute(conn)
    .context("failed to ensure diesel migrations table")?;
  Ok(())
}

#[cfg(feature = "postgres")]
fn connect_postgres(database_url: &str) -> Result<PgConnection> {
  let normalized_url = if database_url.starts_with("postgres://")
    || database_url.starts_with("postgresql://")
  {
    database_url.to_owned()
  } else {
    format!("postgres://{database_url}")
  };

  PgConnection::establish(&normalized_url).with_context(|| {
    format!("failed to connect to database {}", normalized_url)
  })
}

#[cfg(feature = "mysql")]
fn connect_mysql(database_url: &str) -> Result<MysqlConnection> {
  let normalized_url = if database_url.starts_with("mysql://") {
    database_url.to_owned()
  } else {
    format!("mysql://{database_url}")
  };

  MysqlConnection::establish(&normalized_url).with_context(|| {
    format!("failed to connect to database {}", normalized_url)
  })
}

#[cfg(feature = "postgres")]
fn deploy_with_pg(
  conn: &mut PgConnection,
  migrations: Vec<MigrationFile>,
  applied: BTreeMap<String, String>,
) -> Result<usize> {
  let mut applied_count = 0usize;
  for migration in migrations {
    if applied.contains_key(&migration.version) {
      continue;
    }

    sql_query("BEGIN")
      .execute(conn)
      .context("failed to begin postgres transaction")?;
    let statements = split_sql_statements(&migration.sql);
    for statement in statements {
      sql_query(statement).execute(conn).with_context(|| {
        format!(
          "failed to execute migration statement in {}",
          migration.folder
        )
      })?;
    }
    let insert_sql = format!(
      "INSERT INTO {DIESEL_MIGRATIONS_TABLE} (version, run_on) VALUES ($1, $2)"
    );
    let insert_result = sql_query(insert_sql)
      .bind::<Text, _>(&migration.version)
      .bind::<Text, _>(Utc::now().naive_utc().to_string())
      .execute(conn);
    if let Err(err) = insert_result {
      let _ = sql_query("ROLLBACK").execute(conn);
      return Err(anyhow::anyhow!(
        "failed to insert applied migration for {}: {err}",
        migration.folder
      ));
    }
    sql_query("COMMIT")
      .execute(conn)
      .context("failed to commit postgres transaction")?;
    println!("Applied {}", migration.folder);
    applied_count += 1;
  }
  Ok(applied_count)
}

#[cfg(feature = "mysql")]
fn deploy_with_mysql(
  conn: &mut MysqlConnection,
  migrations: Vec<MigrationFile>,
  applied: BTreeMap<String, String>,
) -> Result<usize> {
  let mut applied_count = 0usize;
  for migration in migrations {
    if applied.contains_key(&migration.version) {
      continue;
    }

    sql_query("START TRANSACTION")
      .execute(conn)
      .context("failed to begin mysql transaction")?;
    let statements = split_sql_statements(&migration.sql);
    for statement in statements {
      sql_query(statement).execute(conn).with_context(|| {
        format!(
          "failed to execute migration statement in {}",
          migration.folder
        )
      })?;
    }
    let insert_sql = format!(
      "INSERT INTO {DIESEL_MIGRATIONS_TABLE} (version, run_on) VALUES (?, ?)"
    );
    let insert_result = sql_query(insert_sql)
      .bind::<Text, _>(&migration.version)
      .bind::<Text, _>(Utc::now().naive_utc().to_string())
      .execute(conn);
    if let Err(err) = insert_result {
      let _ = sql_query("ROLLBACK").execute(conn);
      return Err(anyhow::anyhow!(
        "failed to insert applied migration for {}: {err}",
        migration.folder
      ));
    }
    sql_query("COMMIT")
      .execute(conn)
      .context("failed to commit mysql transaction")?;
    println!("Applied {}", migration.folder);
    applied_count += 1;
  }
  Ok(applied_count)
}

#[derive(Debug, QueryableByName)]
struct AppliedMigrationRow {
  #[diesel(sql_type = Text)]
  version: String,
}

#[cfg(feature = "postgres")]
fn load_applied_map_pg(
  conn: &mut PgConnection,
) -> Result<BTreeMap<String, String>> {
  let query = format!("SELECT version FROM {DIESEL_MIGRATIONS_TABLE}");
  let rows: Vec<AppliedMigrationRow> = sql_query(query)
    .load(conn)
    .context("failed to query _prismar_migrations")?;
  Ok(
    rows
      .into_iter()
      .map(|row| (row.version, String::new()))
      .collect(),
  )
}

#[cfg(feature = "mysql")]
fn load_applied_map_mysql(
  conn: &mut MysqlConnection,
) -> Result<BTreeMap<String, String>> {
  let query = format!("SELECT version FROM {DIESEL_MIGRATIONS_TABLE}");
  let rows: Vec<AppliedMigrationRow> = sql_query(query)
    .load(conn)
    .context("failed to query _prismar_migrations")?;
  Ok(
    rows
      .into_iter()
      .map(|row| (row.version, String::new()))
      .collect(),
  )
}

#[derive(Debug, Clone)]
struct MigrationFile {
  folder: String,
  version: String,
  sql: String,
}

fn load_migration_files(migrations_dir: &Path) -> Result<Vec<MigrationFile>> {
  if !migrations_dir.exists() {
    return Ok(Vec::new());
  }

  let mut entries = fs::read_dir(migrations_dir)?
    .filter_map(|entry| entry.ok())
    .filter(|entry| entry.path().is_dir())
    .collect::<Vec<_>>();

  entries.sort_by_key(|entry| entry.file_name());

  let mut files = Vec::new();
  for entry in entries {
    let folder = entry.file_name().to_string_lossy().to_string();
    let migration_file = entry.path().join("migration.sql");
    if !migration_file.exists() {
      continue;
    }

    let sql: String =
      fs::read_to_string(&migration_file).with_context(|| {
        format!("failed to read migration file {}", migration_file.display())
      })?;

    let (version, _name) = split_migration_folder(&folder);
    files.push(MigrationFile {
      folder,
      version,
      sql,
    });
  }

  Ok(files)
}

fn split_migration_folder(folder: &str) -> (String, String) {
  if let Some((version, name)) = folder.split_once('_') {
    (version.to_owned(), name.to_owned())
  } else {
    (folder.to_owned(), folder.to_owned())
  }
}

fn introspect_sqlite_schema(conn: &Connection) -> Result<Schema> {
  let mut stmt = conn.prepare(
    "SELECT name FROM sqlite_master
     WHERE type='table'
       AND name NOT LIKE 'sqlite_%'
       AND name != '__diesel_schema_migrations'
     ORDER BY name",
  )?;
  let table_rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

  let mut models = Vec::new();
  for table_row in table_rows {
    let table_name = table_row?;
    let pragma = format!("PRAGMA table_info('{table_name}')");
    let mut pragma_stmt = conn.prepare(&pragma)?;
    let field_rows = pragma_stmt.query_map([], |row| {
      let name: String = row.get(1)?;
      let sql_type: String = row.get(2)?;
      let not_null: i64 = row.get(3)?;
      let default_value: Option<String> = row.get(4)?;
      let is_pk: i64 = row.get(5)?;
      Ok((name, sql_type, not_null, default_value, is_pk))
    })?;

    let mut fields = Vec::new();
    for field_row in field_rows {
      let (name, sql_type, not_null, default_value, is_pk) = field_row?;
      let mut attributes = Vec::new();
      if is_pk > 0 {
        attributes.push(FieldAttribute::Id);
      }
      if let Some(default_value) = default_value {
        attributes.push(FieldAttribute::Default(default_value));
      }

      fields.push(Field {
        name,
        r#type: sqlite_type_to_field_type(&sql_type),
        optional: not_null == 0,
        array: false,
        attributes,
      });
    }

    models.push(Model {
      name: table_name.clone(),
      mapped_name: Some(table_name),
      fields,
      attributes: Vec::new(),
    });
  }

  Ok(Schema {
    datasource: None,
    generators: Vec::new(),
    models,
  })
}

fn introspect_sql_schema(
  database_url: &str,
  backend: prismar_migrate::SqlBackend,
) -> Result<Schema> {
  match backend {
    prismar_migrate::SqlBackend::Postgres => {
      #[cfg(feature = "postgres")]
      {
        let mut conn = connect_postgres(database_url)?;
        introspect_postgres_schema(&mut conn)
      }
      #[cfg(not(feature = "postgres"))]
      {
        Err(anyhow::anyhow!(
          "postgres support is disabled; recompile with --features postgres"
        ))
      }
    }
    prismar_migrate::SqlBackend::MySql => {
      #[cfg(feature = "mysql")]
      {
        let mut conn = connect_mysql(database_url)?;
        introspect_mysql_schema(&mut conn)
      }
      #[cfg(not(feature = "mysql"))]
      {
        Err(anyhow::anyhow!(
          "mysql support is disabled; recompile with --features mysql"
        ))
      }
    }
    prismar_migrate::SqlBackend::Sqlite => {
      unreachable!("sqlite uses dedicated introspection")
    }
  }
}

#[derive(Debug, QueryableByName)]
struct TableNameRow {
  #[diesel(sql_type = Text)]
  table_name: String,
}

#[cfg(feature = "postgres")]
fn introspect_postgres_schema(conn: &mut PgConnection) -> Result<Schema> {
  let table_rows: Vec<TableNameRow> = sql_query(
    "SELECT table_name FROM information_schema.tables
     WHERE table_schema = 'public'
       AND table_type = 'BASE TABLE'
       AND table_name <> '__diesel_schema_migrations'
     ORDER BY table_name",
  )
  .load(conn)
  .context("failed to list postgres tables")?;

  let mut models = Vec::new();
  for table_row in table_rows {
    let table_name = table_row.table_name;
    let fields = introspect_postgres_columns(conn, &table_name)?;
    models.push(Model {
      name: table_name.clone(),
      mapped_name: Some(table_name),
      fields,
      attributes: Vec::new(),
    });
  }

  Ok(Schema {
    datasource: None,
    generators: Vec::new(),
    models,
  })
}

#[cfg(feature = "mysql")]
fn introspect_mysql_schema(conn: &mut MysqlConnection) -> Result<Schema> {
  let table_rows: Vec<TableNameRow> = sql_query(
    "SELECT table_name FROM information_schema.tables
     WHERE table_schema = DATABASE()
       AND table_type = 'BASE TABLE'
       AND table_name <> '__diesel_schema_migrations'
     ORDER BY table_name",
  )
  .load(conn)
  .context("failed to list mysql tables")?;

  let mut models = Vec::new();
  for table_row in table_rows {
    let table_name = table_row.table_name;
    let fields = introspect_mysql_columns(conn, &table_name)?;
    models.push(Model {
      name: table_name.clone(),
      mapped_name: Some(table_name),
      fields,
      attributes: Vec::new(),
    });
  }

  Ok(Schema {
    datasource: None,
    generators: Vec::new(),
    models,
  })
}

#[derive(Debug, QueryableByName)]
struct PgColumnRow {
  #[diesel(sql_type = Text)]
  column_name: String,
  #[diesel(sql_type = Text)]
  data_type: String,
  #[diesel(sql_type = Text)]
  is_nullable: String,
  #[diesel(sql_type = Nullable<Text>)]
  column_default: Option<String>,
  #[diesel(sql_type = diesel::sql_types::Bool)]
  is_pk: bool,
}

#[cfg(feature = "postgres")]
fn introspect_postgres_columns(
  conn: &mut PgConnection,
  table_name: &str,
) -> Result<Vec<Field>> {
  let rows: Vec<PgColumnRow> = sql_query(
    "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default,
            EXISTS (
              SELECT 1
              FROM information_schema.table_constraints tc
              JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
               AND tc.table_schema = kcu.table_schema
               AND tc.table_name = kcu.table_name
             WHERE tc.table_schema = 'public'
               AND tc.table_name = c.table_name
               AND tc.constraint_type = 'PRIMARY KEY'
               AND kcu.column_name = c.column_name
            ) AS is_pk
     FROM information_schema.columns c
     WHERE c.table_schema = 'public'
       AND c.table_name = $1
     ORDER BY c.ordinal_position",
  )
  .bind::<Text, _>(table_name)
  .load(conn)
  .context("failed to introspect postgres columns")?;

  let mut fields = Vec::new();
  for row in rows {
    let name = row.column_name;
    let data_type = row.data_type;
    let is_nullable = row.is_nullable;
    let default_value = row.column_default;
    let is_pk = row.is_pk;

    let mut attributes = Vec::new();
    if is_pk {
      attributes.push(FieldAttribute::Id);
    }
    if let Some(default_value) = default_value {
      attributes.push(FieldAttribute::Default(default_value));
    }

    fields.push(Field {
      name,
      r#type: sql_type_to_field_type(&data_type),
      optional: is_nullable.eq_ignore_ascii_case("YES"),
      array: false,
      attributes,
    });
  }

  Ok(fields)
}

#[cfg(feature = "mysql")]
#[derive(Debug, QueryableByName)]
struct MySqlColumnRow {
  #[diesel(sql_type = Text)]
  column_name: String,
  #[diesel(sql_type = Text)]
  data_type: String,
  #[diesel(sql_type = Text)]
  is_nullable: String,
  #[diesel(sql_type = Nullable<Text>)]
  column_default: Option<String>,
  #[diesel(sql_type = Nullable<Text>)]
  column_key: Option<String>,
}

#[cfg(feature = "mysql")]
fn introspect_mysql_columns(
  conn: &mut MysqlConnection,
  table_name: &str,
) -> Result<Vec<Field>> {
  let rows: Vec<MySqlColumnRow> = sql_query(
    "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default,
            c.column_key
     FROM information_schema.columns c
     WHERE c.table_schema = DATABASE()
       AND c.table_name = ?
     ORDER BY c.ordinal_position",
  )
  .bind::<Text, _>(table_name)
  .load(conn)
  .context("failed to introspect mysql columns")?;

  let mut fields = Vec::new();
  for row in rows {
    let name = row.column_name;
    let data_type = row.data_type;
    let is_nullable = row.is_nullable;
    let default_value = row.column_default;
    let is_pk = row.column_key.as_deref() == Some("PRI");

    let mut attributes = Vec::new();
    if is_pk {
      attributes.push(FieldAttribute::Id);
    }
    if let Some(default_value) = default_value {
      attributes.push(FieldAttribute::Default(default_value));
    }

    fields.push(Field {
      name,
      r#type: sql_type_to_field_type(&data_type),
      optional: is_nullable.eq_ignore_ascii_case("YES"),
      array: false,
      attributes,
    });
  }

  Ok(fields)
}

fn sql_type_to_field_type(sql_type: &str) -> FieldType {
  let upper = sql_type.trim().to_uppercase();
  if upper.contains("BIGINT") {
    FieldType::BigInt
  } else if upper.contains("INT") {
    FieldType::Int
  } else if upper.contains("DOUBLE")
    || upper.contains("REAL")
    || upper.contains("FLOAT")
  {
    FieldType::Float
  } else if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
    FieldType::Decimal
  } else if upper.contains("BOOL") {
    FieldType::Boolean
  } else if upper.contains("UUID") {
    FieldType::Uuid
  } else if upper.contains("JSON") {
    FieldType::Json
  } else if upper.contains("DATE") || upper.contains("TIME") {
    FieldType::DateTime
  } else if upper.contains("BLOB")
    || upper.contains("BYTEA")
    || upper.contains("BINARY")
  {
    FieldType::Bytes
  } else {
    FieldType::String
  }
}

fn sqlite_type_to_field_type(sql_type: &str) -> FieldType {
  let upper = sql_type.trim().to_uppercase();
  if upper.contains("INT") {
    FieldType::Int
  } else if upper.contains("REAL")
    || upper.contains("DOUBLE")
    || upper.contains("FLOAT")
  {
    FieldType::Float
  } else if upper.contains("BOOL") {
    FieldType::Boolean
  } else if upper.contains("JSON") {
    FieldType::Json
  } else if upper.contains("DATE") || upper.contains("TIME") {
    FieldType::DateTime
  } else if upper.contains("BLOB") {
    FieldType::Bytes
  } else {
    FieldType::String
  }
}

fn sanitize_name(input: String) -> String {
  let out = input
    .chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
        ch
      } else {
        '_'
      }
    })
    .collect::<String>();

  if out.is_empty() {
    "migration".to_owned()
  } else {
    out
  }
}
