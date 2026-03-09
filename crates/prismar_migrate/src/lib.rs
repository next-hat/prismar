use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
pub use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use prismar_schema::{Field, FieldAttribute, FieldType, Model, Schema};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlBackend {
  Postgres,
  MySql,
  Sqlite,
}

#[derive(Debug, Error)]
pub enum MigrationError {
  #[error("unsupported operation for backend {backend:?}: {operation}")]
  Unsupported { backend: SqlBackend, operation: String },
}

pub fn backend_from_provider(provider: &str) -> SqlBackend {
  match provider {
    "postgresql" | "postgres" => SqlBackend::Postgres,
    "mysql" => SqlBackend::MySql,
    "sqlite" => SqlBackend::Sqlite,
    _ => SqlBackend::Postgres,
  }
}

pub fn generate_schema_sql(schema: &Schema, backend: SqlBackend) -> Vec<String> {
  schema
    .models
    .iter()
    .map(|model| render_create_table(schema, model, backend))
    .collect()
}

pub fn diff_schema_sql(old: &Schema, new: &Schema, backend: SqlBackend) -> Vec<String> {
  let mut statements = Vec::new();

  let old_map = old
    .models
    .iter()
    .map(|model| (table_name(model), model))
    .collect::<BTreeMap<_, _>>();

  let new_map = new
    .models
    .iter()
    .map(|model| (table_name(model), model))
    .collect::<BTreeMap<_, _>>();

  for (table, model) in &new_map {
    if !old_map.contains_key(table) {
      statements.push(render_create_table(new, model, backend));
    }
  }

  for table in old_map.keys() {
    if !new_map.contains_key(table) {
      statements.push(format!("DROP TABLE {};", table));
    }
  }

  for (table, new_model) in &new_map {
    let Some(old_model) = old_map.get(table) else {
      continue;
    };

    let old_fields = old_model
      .fields
      .iter()
      .filter(|field| !matches!(field.r#type, FieldType::Relation(_)))
      .map(|field| (field.name.clone(), field))
      .collect::<BTreeMap<_, _>>();
    let new_fields = new_model
      .fields
      .iter()
      .filter(|field| !matches!(field.r#type, FieldType::Relation(_)))
      .map(|field| (field.name.clone(), field))
      .collect::<BTreeMap<_, _>>();

    let old_names = old_fields.keys().cloned().collect::<BTreeSet<_>>();
    let new_names = new_fields.keys().cloned().collect::<BTreeSet<_>>();

    for added in new_names.difference(&old_names) {
      let field = new_fields.get(added).expect("field exists");
      statements.push(format!(
        "ALTER TABLE {} ADD COLUMN {};",
        table,
        render_column(field, backend)
      ));
    }

    for removed in old_names.difference(&new_names) {
      statements.push(render_drop_column(table, removed, backend));
    }
  }

  statements
}

pub fn default_migration_name() -> String {
  Utc::now().format("%Y%m%d%H%M%S").to_string()
}

fn table_name(model: &Model) -> String {
  model
    .mapped_name
    .clone()
    .unwrap_or_else(|| model.name.to_lowercase())
}

fn render_create_table(schema: &Schema, model: &Model, backend: SqlBackend) -> String {
  let table = table_name(model);
  let columns = persisted_fields(model)
    .iter()
    .map(|field| render_column(field, backend))
    .chain(relation_constraints(schema, model).into_iter())
    .collect::<Vec<_>>()
    .join(",\n  ");

  format!("CREATE TABLE {} (\n  {}\n);", table, columns)
}

fn persisted_fields(model: &Model) -> Vec<&Field> {
  model
    .fields
    .iter()
    .filter(|field| !matches!(field.r#type, FieldType::Relation(_)))
    .collect()
}

fn render_column(field: &Field, backend: SqlBackend) -> String {
  let mut parts = vec![field.name.clone(), sql_type(field, backend)];

  if !field.optional {
    parts.push("NOT NULL".to_owned());
  }

  for attr in &field.attributes {
    match attr {
      FieldAttribute::Id => parts.push("PRIMARY KEY".to_owned()),
      FieldAttribute::Unique => parts.push("UNIQUE".to_owned()),
      FieldAttribute::Default(value) => {
        if render_database_default(value) {
          parts.push(format!("DEFAULT {}", rewrite_default(value, backend)))
        }
      }
      FieldAttribute::UpdatedAt => {}
      FieldAttribute::Map(_) | FieldAttribute::Relation(_) | FieldAttribute::Other(_) => {}
    }
  }

  parts.join(" ")
}

fn sql_type(field: &Field, backend: SqlBackend) -> String {
  if field.array {
    return match backend {
      SqlBackend::Postgres => "JSONB".to_owned(),
      SqlBackend::MySql | SqlBackend::Sqlite => "JSON".to_owned(),
    };
  }

  match field.r#type {
    FieldType::Int => "INTEGER".to_owned(),
    FieldType::BigInt => "BIGINT".to_owned(),
    FieldType::Float => "DOUBLE PRECISION".to_owned(),
    FieldType::Decimal => "DECIMAL".to_owned(),
    FieldType::Boolean => "BOOLEAN".to_owned(),
    FieldType::String => "TEXT".to_owned(),
    FieldType::DateTime => "TIMESTAMP".to_owned(),
    FieldType::Json => match backend {
      SqlBackend::Postgres => "JSONB".to_owned(),
      SqlBackend::MySql | SqlBackend::Sqlite => "JSON".to_owned(),
    },
    FieldType::Bytes => "BLOB".to_owned(),
    FieldType::Uuid => match backend {
      SqlBackend::Postgres => "UUID".to_owned(),
      SqlBackend::MySql | SqlBackend::Sqlite => "TEXT".to_owned(),
    },
    FieldType::Relation(_) => "TEXT".to_owned(),
  }
}

fn rewrite_default(value: &str, backend: SqlBackend) -> String {
  match value {
    "autoincrement()" => match backend {
      SqlBackend::Postgres => "GENERATED BY DEFAULT AS IDENTITY".to_owned(),
      SqlBackend::MySql => "AUTO_INCREMENT".to_owned(),
      SqlBackend::Sqlite => "AUTOINCREMENT".to_owned(),
    },
    "now()" => "CURRENT_TIMESTAMP".to_owned(),
    other => other.to_owned(),
  }
}

fn render_database_default(value: &str) -> bool {
  !(value.starts_with("uuid(")
    || value.starts_with("cuid(")
    || value.starts_with("ulid("))
}

fn render_drop_column(table: &str, column: &str, backend: SqlBackend) -> String {
  match backend {
    SqlBackend::Postgres | SqlBackend::MySql => {
      format!("ALTER TABLE {} DROP COLUMN {};", table, column)
    }
    SqlBackend::Sqlite => {
      format!("-- manual action required: SQLite drop column {}.{}", table, column)
    }
  }
}

fn relation_constraints(schema: &Schema, model: &Model) -> Vec<String> {
  model
    .fields
    .iter()
    .filter_map(|field| {
      let FieldType::Relation(parent_model) = &field.r#type else {
        return None;
      };
      let raw = field.attributes.iter().find_map(|attr| match attr {
        FieldAttribute::Relation(raw) => Some(raw.as_str()),
        _ => None,
      })?;
      let fields = extract_relation_list(raw, "fields")?;
      let references = extract_relation_list(raw, "references")?;
      if fields.len() != 1 || references.len() != 1 {
        return None;
      }

      Some(format!(
        "FOREIGN KEY({}) REFERENCES {}({})",
        fields[0],
        parent_model_table(schema, parent_model),
        references[0]
      ))
    })
    .collect()
}

fn parent_model_table(schema: &Schema, parent_model: &str) -> String {
  schema
    .models
    .iter()
    .find(|model| model.name == parent_model)
    .map(table_name)
    .unwrap_or_else(|| parent_model.to_lowercase())
}

fn extract_relation_list(raw: &str, key: &str) -> Option<Vec<String>> {
  let start = raw.find(key)?;
  let after = &raw[start + key.len()..];
  let bracket_start = after.find('[')?;
  let after_bracket = &after[bracket_start + 1..];
  let bracket_end = after_bracket.find(']')?;
  Some(
    after_bracket[..bracket_end]
      .split(',')
      .map(str::trim)
      .filter(|item| !item.is_empty())
      .map(|item| item.trim_matches('"').to_owned())
      .collect(),
  )
}
