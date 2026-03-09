use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use prismar_migrate::{SqlBackend, backend_from_provider};
use prismar_schema::{
  Field, FieldAttribute, FieldType, GeneratorValue, Model, Schema,
  find_generator,
};

const PRISMAR_PROVIDER: &str = "prismar-cli";

pub fn run_generate(
  schema_path: &Path,
  output_override: Option<PathBuf>,
) -> Result<()> {
  let schema = super::load_schema(schema_path)?;
  let output_dir = resolve_output_dir(schema_path, &schema, output_override)?;
  fs::create_dir_all(&output_dir).with_context(|| {
    format!("failed to create output directory {}", output_dir.display())
  })?;

  let schema_rs = render_schema_rs(&schema);
  fs::write(output_dir.join("schema.rs"), schema_rs).with_context(|| {
    format!("failed to write {}", output_dir.join("schema.rs").display())
  })?;

  let mut modules = Vec::new();
  for model in &schema.models {
    let module_name = to_snake_case(&model.name);
    modules.push(module_name.clone());
    let rendered = render_model_file(&schema, model)?;
    fs::write(output_dir.join(format!("{module_name}.rs")), rendered)
      .with_context(|| {
        format!(
          "failed to write {}",
          output_dir.join(format!("{module_name}.rs")).display()
        )
      })?;
  }

  let mod_rs = render_mod_rs(&schema, &modules);
  fs::write(output_dir.join("mod.rs"), mod_rs).with_context(|| {
    format!("failed to write {}", output_dir.join("mod.rs").display())
  })?;

  println!(
    "Generated {} model module(s) in {}",
    schema.models.len(),
    output_dir.display()
  );
  Ok(())
}

fn resolve_output_dir(
  schema_path: &Path,
  schema: &Schema,
  output_override: Option<PathBuf>,
) -> Result<PathBuf> {
  if let Some(path) = output_override {
    return Ok(path);
  }

  if let Some(generator) =
    find_generator(schema, PRISMAR_PROVIDER).or_else(|| {
      schema
        .generators
        .iter()
        .find(|generator| generator.name == "prismar")
    })
    && let Some(output) = &generator.output
  {
    let base = schema_path.parent().unwrap_or_else(|| Path::new("."));
    return Ok(base.join(output));
  }

  Ok(
    schema_path
      .parent()
      .unwrap_or_else(|| Path::new("."))
      .join("generated"),
  )
}

fn render_mod_rs(schema: &Schema, modules: &[String]) -> String {
  let generator = active_generator(schema);
  let mut out = String::from("pub mod schema;\n");
  for module in modules {
    out.push_str(format!("pub mod {module};\npub use {module}::*;\n").as_str());
  }

  if config_bool(generator, "generate_json_types", true) {
    out.push_str("\n#[allow(dead_code)]\npub type PrismaWhereInput = prismar::PrismaWhereInput;\n");
    out.push_str("#[allow(dead_code)]\npub type PrismaReadManyInput = prismar::PrismaReadManyInput;\n");
  }

  out
}

fn render_schema_rs(schema: &Schema) -> String {
  let mut out = String::new();
  let backend = schema_backend(schema);

  for model in &schema.models {
    let table_name = table_name(model);
    let pk = primary_keys(model);
    out.push_str("prismar::diesel::table! {\n");
    out.push_str(
      format!("    {} ({}) {{\n", table_name, pk.join(", ")).as_str(),
    );
    for field in persisted_fields(model) {
      out.push_str(
        format!(
          "        {} -> {},\n",
          column_name(field),
          diesel_sql_type(field, backend)
        )
        .as_str(),
      );
    }
    out.push_str("    }\n}\n\n");
  }

  for model in &schema.models {
    for relation in relation_metadata(model) {
      if relation.self_fields.len() == 1 {
        let child_table = table_name(model);
        let parent_table = schema
          .models
          .iter()
          .find(|item| item.name == relation.other_model)
          .map(table_name)
          .unwrap_or_else(|| to_snake_case(&relation.other_model));
        out.push_str(
          format!(
            "prismar::diesel::joinable!({child_table} -> {parent_table} ({}));\n",
            relation.self_fields[0]
          )
          .as_str(),
        );
      }
    }
  }

  let tables = schema.models.iter().map(table_name).collect::<Vec<_>>();
  if !tables.is_empty() {
    out.push_str("\nprismar::diesel::allow_tables_to_appear_in_same_query!(\n");
    for table in &tables {
      out.push_str(format!("    {},\n", table).as_str());
    }
    out.push_str(");\n");
  }

  out
}

fn render_model_file(schema: &Schema, model: &Model) -> Result<String> {
  let generator = active_generator(schema);
  let table_name = table_name(model);
  let model_name = &model.name;
  let db_name = format!("{}Db", model_name);
  let partial_name = format!("{}Partial", model_name);
  let update_name = format!("{}Update", model_name);
  let create_name = format!("{}Create", model_name);
  let filter_name = format!("{}Filter", db_name);
  let persisted = persisted_fields(model);
  let all_relations = all_relation_metadata(schema, model);
  let pk = primary_keys(model);
  let updatable = persisted
    .iter()
    .filter(|field| !pk.contains(&field.name) && !has_updated_at(field))
    .collect::<Vec<_>>();
  if pk.is_empty() {
    return Err(anyhow!(
      "model {} must define at least one @id or @@id field",
      model.name
    ));
  }

  let mut out = String::new();
  out.push_str("use prismar::PrismaModel;\n");
  out.push_str("use prismar::diesel::{OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};\n\n");
  let mut imported_relations = BTreeSet::new();
  for relation in &all_relations {
    if imported_relations.insert(relation.other_model.clone()) {
      out
        .push_str(format!("use super::{}Db;\n", relation.other_model).as_str());
    }
  }
  if !all_relations.is_empty() {
    out.push('\n');
  }

  let mut db_derives = default_db_derives();
  if !relation_metadata(model).is_empty() {
    db_derives.push("prismar::diesel::Associations".to_owned());
  }
  out.push_str(
    format!(
      "#[derive({})]\n",
      derive_list(db_derives, config_list(generator, "db_derives"))
    )
    .as_str(),
  );
  out.push_str(
    format!("#[diesel(table_name = super::schema::{table_name})]\n").as_str(),
  );
  out.push_str(format!("#[diesel(primary_key({}))]\n", pk.join(", ")).as_str());
  for relation in relation_metadata(model) {
    if relation.self_fields.len() == 1 && relation.kind == RelationKind::ToOne {
      out.push_str(
        format!(
          "#[diesel(belongs_to({}Db, foreign_key = {}))]\n",
          relation.other_model, relation.self_fields[0]
        )
        .as_str(),
      );
    }
  }
  out.push_str(format!("pub struct {db_name} {{\n").as_str());
  for field in &persisted {
    out.push_str(
      format!("  pub {}: {},\n", field.name, rust_type(field, false)).as_str(),
    );
  }
  out.push_str("}\n\n");

  out.push_str(
    format!(
      "#[derive({})]\n",
      derive_list(
        default_partial_derives(),
        config_list(generator, "partial_derives")
      )
    )
    .as_str(),
  );
  out.push_str(
    format!("#[diesel(table_name = super::schema::{table_name})]\n").as_str(),
  );
  out.push_str(format!("pub struct {partial_name} {{\n").as_str());
  for field in &persisted {
    if has_updated_at(field) {
      continue;
    }
    out.push_str(
      format!(
        "  pub {}: Option<{}>,\n",
        field.name,
        rust_type(field, true)
      )
      .as_str(),
    );
  }
  out.push_str("}\n\n");

  if updatable.is_empty() {
    out.push_str(
      format!(
        "#[derive({})]\n",
        derive_list(
          default_empty_update_derives(),
          config_list(generator, "update_derives")
        )
      )
      .as_str(),
    );
    out.push_str("#[allow(dead_code)]\n");
    out.push_str(format!("pub struct {update_name};\n\n").as_str());
  } else {
    out.push_str(
      format!(
        "#[derive({})]\n",
        derive_list(
          default_update_derives(),
          config_list(generator, "update_derives")
        )
      )
      .as_str(),
    );
    out.push_str(
      format!("#[diesel(table_name = super::schema::{table_name})]\n").as_str(),
    );
    out.push_str("#[allow(dead_code)]\n");
    out.push_str(format!("pub struct {update_name} {{\n").as_str());
    for field in &updatable {
      out.push_str(
        format!(
          "  pub {}: Option<{}>,\n",
          field.name,
          rust_type(field, true)
        )
        .as_str(),
      );
    }
    out.push_str("}\n\n");
  }

  out.push_str("#[allow(dead_code)]\n");
  out.push_str(format!("pub type {create_name} = {partial_name};\n").as_str());
  render_relation_loader_impls(schema, model, &mut out)?;
  render_relation_filter_methods(schema, model, &filter_name, &mut out);
  render_model_traits(schema, model, &mut out)?;
  Ok(out)
}

fn render_relation_loader_impls(
  schema: &Schema,
  model: &Model,
  out: &mut String,
) -> Result<()> {
  let db_name = format!("{}Db", model.name);
  let relations = all_relation_metadata(schema, model);
  let mut wrote_impl = false;

  for relation in relations {
    if relation.self_fields.len() != 1 || relation.other_fields.len() != 1 {
      continue;
    }

    if !wrote_impl {
      out.push('\n');
      out.push_str(format!("impl {db_name} {{\n").as_str());
      wrote_impl = true;
    }

    let self_field = &relation.self_fields[0];
    let other_field = &relation.other_fields[0];
    match relation.kind {
      RelationKind::ToOne => {
        out.push_str(
          format!(
            "  pub async fn {}(&self, client: &prismar::PrismaClient) -> Result<Option<{}Db>, prismar::RuntimeError> {{\n",
            relation.field_name, relation.other_model
          )
          .as_str(),
        );
        out.push_str(
          format!(
            "    let related = std::boxed::Box::pin({}Db::find_many(client, None)).await?;\n",
            relation.other_model
          )
          .as_str(),
        );
        out.push_str("    for row in related {\n");
        out.push_str(
          format!(
            "      if row.{other_field} == self.{self_field} {{\n        return Ok(Some(row));\n      }}\n"
          )
          .as_str(),
        );
        out.push_str("    }\n    Ok(None)\n  }\n\n");
      }
      RelationKind::ToMany => {
        out.push_str(
          format!(
            "  pub async fn {}(&self, client: &prismar::PrismaClient, filter: Option<prismar::ModelFilter>) -> Result<Vec<{}Db>, prismar::RuntimeError> {{\n",
            relation.field_name, relation.other_model
          )
          .as_str(),
        );
        out.push_str(
          format!(
            "    let related = std::boxed::Box::pin({}Db::find_many(client, filter)).await?;\n",
            relation.other_model
          )
          .as_str(),
        );
        out.push_str("    let mut matches = Vec::new();\n");
        out.push_str("    for row in related {\n");
        out.push_str(
          format!(
            "      if row.{other_field} == self.{self_field} {{\n        matches.push(row);\n      }}\n"
          )
          .as_str(),
        );
        out.push_str("    }\n    Ok(matches)\n  }\n\n");
      }
    }
  }

  if wrote_impl {
    out.push_str("}\n");
  }

  Ok(())
}

fn render_relation_filter_methods(
  schema: &Schema,
  model: &Model,
  filter_name: &str,
  out: &mut String,
) {
  let relations = all_relation_metadata(schema, model);
  if relations.is_empty() {
    return;
  }

  out.push('\n');
  out.push_str(format!("impl {filter_name} {{\n").as_str());
  for relation in relations {
    let field_name = relation.field_name;
    match relation.kind {
      RelationKind::ToOne => {
        out.push_str(
          format!(
            "  pub fn {field_name}_is<T: prismar::TypedFilter>(mut self, filter: T) -> Self {{\n    self.inner = self.inner.relation(\"{field_name}\", prismar::RelationFilterOp::Is, filter.into_model_filter());\n    self\n  }}\n\n"
          )
          .as_str(),
        );
        out.push_str(
          format!(
            "  pub fn {field_name}_is_not<T: prismar::TypedFilter>(mut self, filter: T) -> Self {{\n    self.inner = self.inner.relation(\"{field_name}\", prismar::RelationFilterOp::IsNot, filter.into_model_filter());\n    self\n  }}\n\n"
          )
          .as_str(),
        );
      }
      RelationKind::ToMany => {
        out.push_str(
          format!(
            "  pub fn {field_name}_some<T: prismar::TypedFilter>(mut self, filter: T) -> Self {{\n    self.inner = self.inner.relation(\"{field_name}\", prismar::RelationFilterOp::Some, filter.into_model_filter());\n    self\n  }}\n\n"
          )
          .as_str(),
        );
        out.push_str(
          format!(
            "  pub fn {field_name}_every<T: prismar::TypedFilter>(mut self, filter: T) -> Self {{\n    self.inner = self.inner.relation(\"{field_name}\", prismar::RelationFilterOp::Every, filter.into_model_filter());\n    self\n  }}\n\n"
          )
          .as_str(),
        );
        out.push_str(
          format!(
            "  pub fn {field_name}_none<T: prismar::TypedFilter>(mut self, filter: T) -> Self {{\n    self.inner = self.inner.relation(\"{field_name}\", prismar::RelationFilterOp::None, filter.into_model_filter());\n    self\n  }}\n\n"
          )
          .as_str(),
        );
      }
    }
  }
  out.push_str("}\n");
}

fn render_model_create_normalizer(
  _schema: &Schema,
  model: &Model,
  out: &mut String,
) -> Result<()> {
  out.push_str("  fn apply_defaults(&mut self) {\n");

  let mut has_assignments = false;
  for field in persisted_fields(model) {
    if let Some(expr) = client_default_expr(field) {
      has_assignments = true;
      out.push_str(
        format!(
          "    if self.{}.is_none() {{\n      self.{} = Some({expr});\n    }}\n",
          field.name,
          field.name,
        )
        .as_str(),
      );
    }
  }

  if !has_assignments {
    out.push_str("    let _ = self;\n");
  }
  out.push_str("  }\n");
  Ok(())
}

fn active_generator(schema: &Schema) -> Option<&prismar_schema::Generator> {
  find_generator(schema, PRISMAR_PROVIDER).or_else(|| {
    schema
      .generators
      .iter()
      .find(|generator| generator.name == "prismar")
  })
}

fn persisted_fields(model: &Model) -> Vec<&Field> {
  model
    .fields
    .iter()
    .filter(|field| !matches!(field.r#type, FieldType::Relation(_)))
    .collect()
}

fn single_primary_key_field(model: &Model) -> Option<&Field> {
  let keys = primary_keys(model);
  if keys.len() != 1 {
    return None;
  }
  let key = &keys[0];
  model.fields.iter().find(|field| field.name == *key)
}

fn has_updatable_fields(model: &Model) -> bool {
  let pk = primary_keys(model);
  persisted_fields(model)
    .iter()
    .any(|field| !pk.contains(&field.name) && !has_updated_at(field))
}

fn primary_keys(model: &Model) -> Vec<String> {
  let mut keys = model
    .fields
    .iter()
    .filter(|field| {
      field
        .attributes
        .iter()
        .any(|attr| matches!(attr, FieldAttribute::Id))
    })
    .map(|field| field.name.clone())
    .collect::<Vec<_>>();
  if !keys.is_empty() {
    return keys;
  }
  for attribute in &model.attributes {
    if let prismar_schema::ModelAttribute::Id(raw) = attribute {
      keys = parse_name_list(raw);
      break;
    }
  }
  keys
}

fn table_name(model: &Model) -> String {
  model
    .mapped_name
    .clone()
    .unwrap_or_else(|| to_snake_case(&model.name))
}

fn column_name(field: &Field) -> String {
  field
    .attributes
    .iter()
    .find_map(|attr| match attr {
      FieldAttribute::Map(name) => Some(name.clone()),
      _ => None,
    })
    .unwrap_or_else(|| field.name.clone())
}

fn diesel_sql_type(field: &Field, _backend: SqlBackend) -> String {
  let base = match field.r#type {
    FieldType::Int => "Integer",
    FieldType::BigInt => "BigInt",
    FieldType::Float => "Double",
    FieldType::Decimal => "Double",
    FieldType::Boolean => "Bool",
    FieldType::String => "Text",
    FieldType::DateTime => "Timestamp",
    FieldType::Json => "Text",
    FieldType::Bytes => "Binary",
    FieldType::Uuid => "Text",
    FieldType::Relation(_) => "Text",
  };
  if field.optional {
    format!("Nullable<{base}>")
  } else {
    base.to_owned()
  }
}

fn rust_type(field: &Field, strip_optional: bool) -> String {
  let base = match field.r#type {
    FieldType::Int => "i32",
    FieldType::BigInt => "i64",
    FieldType::Float => "f64",
    FieldType::Decimal => "f64",
    FieldType::Boolean => "bool",
    FieldType::String => "String",
    FieldType::DateTime => "chrono::NaiveDateTime",
    FieldType::Json => "serde_json::Value",
    FieldType::Bytes => "Vec<u8>",
    FieldType::Uuid => "String",
    FieldType::Relation(_) => "String",
  };
  if field.optional && !strip_optional {
    format!("Option<{base}>")
  } else {
    base.to_owned()
  }
}

fn client_default_expr(field: &Field) -> Option<String> {
  let raw = field.attributes.iter().find_map(|attr| match attr {
    FieldAttribute::Default(value) => Some(value.as_str()),
    _ => None,
  })?;

  if raw.starts_with("uuid(") {
    return Some("uuid::Uuid::new_v4().to_string()".to_owned());
  }

  if raw.starts_with("now(") {
    return Some("chrono::Utc::now().naive_utc()".to_owned());
  }

  None
}

fn default_db_derives() -> Vec<String> {
  vec![
    "Debug".to_owned(),
    "Clone".to_owned(),
    "serde::Serialize".to_owned(),
    "serde::Deserialize".to_owned(),
    "prismar::PrismarModel".to_owned(),
    "prismar::diesel::Queryable".to_owned(),
    "prismar::diesel::Selectable".to_owned(),
    "prismar::diesel::Insertable".to_owned(),
    "prismar::diesel::Identifiable".to_owned(),
  ]
}

fn default_partial_derives() -> Vec<String> {
  vec![
    "Debug".to_owned(),
    "Clone".to_owned(),
    "Default".to_owned(),
    "serde::Serialize".to_owned(),
    "serde::Deserialize".to_owned(),
    "prismar::diesel::Insertable".to_owned(),
  ]
}

fn default_update_derives() -> Vec<String> {
  vec![
    "Debug".to_owned(),
    "Clone".to_owned(),
    "Default".to_owned(),
    "serde::Serialize".to_owned(),
    "serde::Deserialize".to_owned(),
    "prismar::diesel::AsChangeset".to_owned(),
  ]
}

fn default_empty_update_derives() -> Vec<String> {
  vec![
    "Debug".to_owned(),
    "Clone".to_owned(),
    "Default".to_owned(),
    "serde::Serialize".to_owned(),
    "serde::Deserialize".to_owned(),
  ]
}

fn derive_list(defaults: Vec<String>, extras: Vec<String>) -> String {
  let mut items = defaults;
  let mut seen = items.iter().cloned().collect::<BTreeSet<_>>();
  for item in extras {
    if seen.insert(item.clone()) {
      items.push(item);
    }
  }
  items.join(", ")
}

fn config_list(
  generator: Option<&prismar_schema::Generator>,
  key: &str,
) -> Vec<String> {
  generator
    .and_then(|generator| generator.config.get(key))
    .and_then(|value| match value {
      GeneratorValue::List(items) => Some(items.clone()),
      GeneratorValue::String(item) => Some(vec![item.clone()]),
      GeneratorValue::Bool(_) => None,
    })
    .unwrap_or_default()
}

fn config_bool(
  generator: Option<&prismar_schema::Generator>,
  key: &str,
  default: bool,
) -> bool {
  generator
    .and_then(|generator| generator.config.get(key))
    .and_then(|value| match value {
      GeneratorValue::Bool(value) => Some(*value),
      _ => None,
    })
    .unwrap_or(default)
}

fn has_updated_at(field: &Field) -> bool {
  field
    .attributes
    .iter()
    .any(|attr| matches!(attr, FieldAttribute::UpdatedAt))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RelationKind {
  ToOne,
  ToMany,
}

struct RelationMetadata {
  field_name: String,
  other_model: String,
  self_fields: Vec<String>,
  other_fields: Vec<String>,
  kind: RelationKind,
}

fn relation_metadata(model: &Model) -> Vec<RelationMetadata> {
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
      Some(RelationMetadata {
        field_name: field.name.clone(),
        other_model: parent_model.clone(),
        self_fields: fields,
        other_fields: references,
        kind: if field.array {
          RelationKind::ToMany
        } else {
          RelationKind::ToOne
        },
      })
    })
    .collect()
}

fn all_relation_metadata(
  schema: &Schema,
  model: &Model,
) -> Vec<RelationMetadata> {
  let mut relations = relation_metadata(model);

  for field in &model.fields {
    let FieldType::Relation(other_model) = &field.r#type else {
      continue;
    };
    if !field.array {
      continue;
    }
    if field
      .attributes
      .iter()
      .any(|attr| matches!(attr, FieldAttribute::Relation(_)))
    {
      continue;
    }

    let Some(other_model_schema) =
      schema.models.iter().find(|item| item.name == *other_model)
    else {
      continue;
    };
    let Some(inverse) =
      other_model_schema.fields.iter().find_map(|other_field| {
        let FieldType::Relation(target_model) = &other_field.r#type else {
          return None;
        };
        if target_model != &model.name {
          return None;
        }
        let raw =
          other_field.attributes.iter().find_map(|attr| match attr {
            FieldAttribute::Relation(raw) => Some(raw.as_str()),
            _ => None,
          })?;
        let child_fields = extract_relation_list(raw, "fields")?;
        let parent_fields = extract_relation_list(raw, "references")?;
        Some((child_fields, parent_fields))
      })
    else {
      continue;
    };

    relations.push(RelationMetadata {
      field_name: field.name.clone(),
      other_model: other_model.clone(),
      self_fields: inverse.1,
      other_fields: inverse.0,
      kind: RelationKind::ToMany,
    });
  }

  relations
}

fn extract_relation_list(raw: &str, key: &str) -> Option<Vec<String>> {
  let start = raw.find(key)?;
  let after = &raw[start + key.len()..];
  let bracket_start = after.find('[')?;
  let after_bracket = &after[bracket_start + 1..];
  let bracket_end = after_bracket.find(']')?;
  Some(parse_name_list(&after_bracket[..bracket_end]))
}

fn parse_name_list(raw: &str) -> Vec<String> {
  raw
    .split(',')
    .map(str::trim)
    .filter(|item| !item.is_empty())
    .map(|item| item.trim_matches('"').to_owned())
    .collect()
}

fn to_snake_case(input: &str) -> String {
  let mut out = String::new();
  for (index, ch) in input.chars().enumerate() {
    if ch.is_uppercase() {
      if index > 0 {
        out.push('_');
      }
      for lower in ch.to_lowercase() {
        out.push(lower);
      }
    } else {
      out.push(ch);
    }
  }
  out
}

fn schema_backend(schema: &Schema) -> SqlBackend {
  backend_from_provider(
    schema
      .datasource
      .as_ref()
      .map(|datasource| datasource.provider.as_str())
      .unwrap_or("postgresql"),
  )
}

fn render_model_traits(
  schema: &Schema,
  model: &Model,
  out: &mut String,
) -> Result<()> {
  let model_name = &model.name;
  let db_name = format!("{}Db", model_name);
  let create_name = format!("{}Create", model_name);
  let update_name = format!("{}Update", model_name);
  let table = table_name(model);
  let persisted = persisted_fields(model);

  out.push('\n');
  out.push_str(
    format!("impl prismar::PrismaCreateData for {create_name} {{\n").as_str(),
  );
  out.push_str(format!("  type Model = {db_name};\n\n").as_str());
  render_model_create_normalizer(schema, model, out)?;
  out.push_str("}\n\n");

  out.push_str(
    format!("impl prismar::PrismaUpdateData for {update_name} {{\n").as_str(),
  );
  out.push_str(format!("  type Model = {db_name};\n").as_str());
  out.push_str("}\n\n");

  let Some(pk_field) = single_primary_key_field(model) else {
    return Ok(());
  };
  let pk_type = rust_type(pk_field, false);

  out
    .push_str(format!("impl prismar::PrismaModel for {db_name} {{\n").as_str());
  out.push_str(format!("  type Create = {create_name};\n").as_str());
  out.push_str(format!("  type Update = {update_name};\n").as_str());
  out.push_str(format!("  type Id = {pk_type};\n\n").as_str());
  out.push_str(
    format!(
      "  fn primary_key_field() -> &'static str {{ \"{}\" }}\n\n",
      pk_field.name
    )
    .as_str(),
  );
  out.push_str("  fn id(&self) -> Self::Id {\n");
  out.push_str(format!("    self.{}.clone()\n", pk_field.name).as_str());
  out.push_str("  }\n\n");
  out.push_str("  fn id_from_filter(filter: prismar::ModelFilter) -> Result<Self::Id, prismar::RuntimeError> {\n");
  out.push_str(format!("    let expected = \"{}\";\n", pk_field.name).as_str());
  out.push_str("    if filter.conditions.len() != 1 {\n      return Err(prismar::RuntimeError::InvalidFilter(format!(\"expected a single equality predicate on {}\", expected)));\n    }\n\n");
  out.push_str("    match filter.conditions.into_iter().next().expect(\"one condition\") {\n");
  out.push_str("      prismar::Condition::Predicate(predicate) if predicate.field == expected => {\n");
  out.push_str(
    format!(
      "        {}\n",
      parse_id_from_field_filter(pk_field).trim_end()
    )
    .as_str(),
  );
  out.push_str("      }\n");
  out.push_str("      _ => Err(prismar::RuntimeError::InvalidFilter(format!(\"expected equality predicate on {}\", expected))),\n");
  out.push_str("    }\n  }\n\n");
  out.push_str(
    "  fn id_from_create(data: &Self::Create) -> Option<Self::Id> {\n",
  );
  out.push_str(format!("    {}\n", render_create_id_access(pk_field)).as_str());
  out.push_str("  }\n\n");
  out.push_str("  async fn matches_filter(&self, client: &prismar::PrismaClient, filter: &prismar::ModelFilter) -> Result<bool, prismar::RuntimeError> {\n");
  out.push_str("    for condition in &filter.conditions {\n");
  out.push_str("      let matched = match condition {\n");
  out.push_str("        prismar::Condition::Predicate(predicate) => match predicate.field.as_str() {\n");
  for field in &persisted {
    out.push_str(render_predicate_match_arm(field).as_str());
  }
  out.push_str("          unknown => Err(prismar::RuntimeError::InvalidFilter(format!(\"unknown field '{}'\", unknown))),\n");
  out.push_str("        }?,\n");
  out.push_str("        prismar::Condition::And(filters) => {\n");
  out.push_str("          let mut all_match = true;\n");
  out.push_str("          for inner in filters {\n");
  out.push_str("            if !std::boxed::Box::pin(self.matches_filter(client, inner)).await? {\n");
  out.push_str("              all_match = false;\n");
  out.push_str("              break;\n");
  out.push_str("            }\n");
  out.push_str("          }\n");
  out.push_str("          all_match\n");
  out.push_str("        }\n");
  out.push_str("        prismar::Condition::Or(filters) => {\n");
  out.push_str("          let mut any_match = false;\n");
  out.push_str("          for inner in filters {\n");
  out.push_str("            if std::boxed::Box::pin(self.matches_filter(client, inner)).await? {\n");
  out.push_str("              any_match = true;\n");
  out.push_str("              break;\n");
  out.push_str("            }\n");
  out.push_str("          }\n");
  out.push_str("          any_match\n");
  out.push_str("        }\n");
  out.push_str(
    "        prismar::Condition::Not(inner) => !std::boxed::Box::pin(self.matches_filter(client, inner)).await?,\n",
  );
  out.push_str("        prismar::Condition::Relation(relation) => match relation.field.as_str() {\n");
  for relation in all_relation_metadata(schema, model) {
    if relation.self_fields.len() == 1 && relation.other_fields.len() == 1 {
      out.push_str(render_relation_match_arm(&relation).as_str());
    }
  }
  out.push_str("          unknown => Err(prismar::RuntimeError::InvalidFilter(format!(\"unknown relation '{}'\", unknown))),\n");
  out.push_str("        }?,\n");
  out.push_str("      };\n");
  out.push_str("      if !matched {\n");
  out.push_str("        return Ok(false);\n");
  out.push_str("      }\n");
  out.push_str("    }\n");
  out.push_str("    Ok(true)\n");
  out.push_str("  }\n\n");

  out.push_str(
    "  async fn create(client: &prismar::PrismaClient, data: Self::Create) -> Result<usize, prismar::RuntimeError> {\n",
  );
  render_backend_dispatch(schema, out, |backend| {
    format!(
      "      client.{backend}(move |conn| {{ diesel::insert_into(super::schema::{table}::table).values(&data).execute(conn) }}).await\n"
    )
  });
  out.push_str("  }\n\n");

  out.push_str(
    "  async fn find_many(client: &prismar::PrismaClient, filter: Option<prismar::ModelFilter>) -> Result<Vec<Self>, prismar::RuntimeError> {\n",
  );
  out.push_str("    let rows = ");
  out.push_str(backend_dispatch_expr(schema, |backend| {
    format!(
      "      client.{backend}(|conn| {{ super::schema::{table}::table.select(Self::as_select()).load::<Self>(conn) }}).await\n"
    )
  }).as_str());
  out.push_str(";\n");
  out.push_str("    let mut rows = rows?;\n");
  out.push_str("    if let Some(filter) = filter {\n");
  out.push_str("      let mut filtered = Vec::new();\n");
  out.push_str("      for row in rows.drain(..) {\n");
  out.push_str("        if row.matches_filter(client, &filter).await? {\n");
  out.push_str("          filtered.push(row);\n");
  out.push_str("        }\n");
  out.push_str("      }\n");
  out.push_str("      return Ok(filtered);\n");
  out.push_str("    }\n");
  out.push_str("    Ok(rows)\n");
  out.push_str("  }\n\n");

  out.push_str(
    "  async fn find_by_id(client: &prismar::PrismaClient, id: &Self::Id) -> Result<Option<Self>, prismar::RuntimeError> {\n",
  );
  out.push_str("    let id = id.clone();\n");
  render_backend_dispatch(schema, out, |backend| {
    format!(
      "      client.{backend}(move |conn| {{ super::schema::{table}::table.find(id).select(Self::as_select()).first::<Self>(conn).optional() }}).await\n"
    )
  });
  out.push_str("  }\n\n");

  out.push_str(
    "  async fn update_by_id(client: &prismar::PrismaClient, id: &Self::Id, data: Self::Update) -> Result<usize, prismar::RuntimeError> {\n",
  );
  if has_updatable_fields(model) {
    out.push_str("    let id = id.clone();\n");
    render_backend_dispatch(schema, out, |backend| {
      format!(
        "      client.{backend}(move |conn| {{ diesel::update(super::schema::{table}::table.find(id)).set(&data).execute(conn) }}).await\n"
      )
    });
  } else {
    out.push_str("    let _ = (client, id, data);\n    Ok(0)\n");
  }
  out.push_str("  }\n\n");

  out.push_str(
    "  async fn delete_by_id(client: &prismar::PrismaClient, id: &Self::Id) -> Result<usize, prismar::RuntimeError> {\n",
  );
  out.push_str("    let id = id.clone();\n");
  render_backend_dispatch(schema, out, |backend| {
    format!(
      "      client.{backend}(move |conn| {{ diesel::delete(super::schema::{table}::table.find(id)).execute(conn) }}).await\n"
    )
  });
  out.push_str("  }\n");
  out.push_str("}\n");

  Ok(())
}

fn render_backend_dispatch<F>(schema: &Schema, out: &mut String, render: F)
where
  F: Fn(&str) -> String,
{
  out.push_str(backend_dispatch_expr(schema, render).as_str());
}

fn backend_dispatch_expr<F>(schema: &Schema, render: F) -> String
where
  F: Fn(&str) -> String,
{
  let _ = schema;
  let mut out = String::new();
  out.push_str("match client.provider() {\n");
  out.push_str("      prismar::Provider::Sqlite => {\n");
  out.push_str("        #[cfg(feature = \"sqlite\")] {\n");
  out.push_str(render("run_sqlite").as_str());
  out.push_str("        }\n        #[cfg(not(feature = \"sqlite\"))] { Err(prismar::RuntimeError::UnsupportedProvider(\"sqlite\")) }\n      }\n");
  out.push_str("      prismar::Provider::Postgres => {\n");
  out.push_str("        #[cfg(feature = \"postgres\")] {\n");
  out.push_str(render("run_postgres").as_str());
  out.push_str("        }\n        #[cfg(not(feature = \"postgres\"))] { Err(prismar::RuntimeError::UnsupportedProvider(\"postgres\")) }\n      }\n");
  out.push_str("      prismar::Provider::MySql => {\n");
  out.push_str("        #[cfg(feature = \"mysql\")] {\n");
  out.push_str(render("run_mysql").as_str());
  out.push_str("        }\n        #[cfg(not(feature = \"mysql\"))] { Err(prismar::RuntimeError::UnsupportedProvider(\"mysql\")) }\n      }\n");
  out.push_str("    }");
  out
}

fn render_create_id_access(field: &Field) -> String {
  let accessor = format!("data.{}.clone()", field.name);
  match field.r#type {
    FieldType::String
    | FieldType::Uuid
    | FieldType::Relation(_)
    | FieldType::Int
    | FieldType::BigInt
    | FieldType::Float
    | FieldType::Decimal
    | FieldType::Boolean
    | FieldType::DateTime => accessor,
    FieldType::Json | FieldType::Bytes => "None".to_owned(),
  }
}

fn render_predicate_match_arm(field: &Field) -> String {
  let expr = match field.r#type {
    FieldType::String | FieldType::Uuid | FieldType::Relation(_) => {
      if field.optional {
        format!(
          "prismar::evaluate_string_field(self.{}.as_deref(), &predicate.filter)",
          field.name
        )
      } else {
        format!(
          "prismar::evaluate_string_field(Some(self.{}.as_str()), &predicate.filter)",
          field.name
        )
      }
    }
    FieldType::Int
    | FieldType::BigInt
    | FieldType::Float
    | FieldType::Decimal => {
      if field.optional {
        format!(
          "prismar::evaluate_number_field(self.{}.map(|value| value as f64), &predicate.filter)",
          field.name
        )
      } else {
        format!(
          "prismar::evaluate_number_field(Some(self.{} as f64), &predicate.filter)",
          field.name
        )
      }
    }
    FieldType::Boolean => {
      if field.optional {
        format!(
          "prismar::evaluate_bool_field(self.{}, &predicate.filter)",
          field.name
        )
      } else {
        format!(
          "prismar::evaluate_bool_field(Some(self.{}), &predicate.filter)",
          field.name
        )
      }
    }
    FieldType::DateTime => {
      if field.optional {
        format!(
          "prismar::evaluate_datetime_field(self.{}, &predicate.filter)",
          field.name
        )
      } else {
        format!(
          "prismar::evaluate_datetime_field(Some(self.{}), &predicate.filter)",
          field.name
        )
      }
    }
    FieldType::Json => {
      if field.optional {
        format!(
          "prismar::evaluate_json_field(self.{}.as_ref(), &predicate.filter)",
          field.name
        )
      } else {
        format!(
          "prismar::evaluate_json_field(Some(&self.{}), &predicate.filter)",
          field.name
        )
      }
    }
    FieldType::Bytes => {
      "Err(prismar::RuntimeError::InvalidFilter(\"bytes filters are not supported yet\".to_owned()))".to_owned()
    }
  };

  format!("          \"{}\" => {},\n", field.name, expr)
}

fn render_relation_match_arm(relation: &RelationMetadata) -> String {
  let field_name = &relation.field_name;
  match relation.kind {
    RelationKind::ToOne => format!(
      "          \"{field_name}\" => {{\n            let related = std::boxed::Box::pin(self.{field_name}(client)).await?;\n            match relation.op {{\n              prismar::RelationFilterOp::Is | prismar::RelationFilterOp::Some | prismar::RelationFilterOp::Every => match related {{\n                Some(related) => std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await,\n                None => Ok(false),\n              }},\n              prismar::RelationFilterOp::IsNot | prismar::RelationFilterOp::None => match related {{\n                Some(related) => Ok(!std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await?),\n                None => Ok(true),\n              }},\n            }}\n          }},\n"
    ),
    RelationKind::ToMany => format!(
      "          \"{field_name}\" => {{\n            let related = std::boxed::Box::pin(self.{field_name}(client, None)).await?;\n            match relation.op {{\n              prismar::RelationFilterOp::Some => {{\n                let mut matched = false;\n                for related in related {{\n                  if std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {{\n                    matched = true;\n                    break;\n                  }}\n                }}\n                Ok(matched)\n              }}\n              prismar::RelationFilterOp::Every => {{\n                let mut matched = true;\n                for related in related {{\n                  if !std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {{\n                    matched = false;\n                    break;\n                  }}\n                }}\n                Ok(matched)\n              }}\n              prismar::RelationFilterOp::None => {{\n                let mut matched = true;\n                for related in related {{\n                  if std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {{\n                    matched = false;\n                    break;\n                  }}\n                }}\n                Ok(matched)\n              }}\n              prismar::RelationFilterOp::Is | prismar::RelationFilterOp::IsNot => Err(prismar::RuntimeError::InvalidFilter(format!(\"relation '{field_name}' is to-many; use some/every/none\"))),\n            }}\n          }},\n"
    ),
  }
}

fn parse_id_from_field_filter(field: &Field) -> String {
  match field.r#type {
    FieldType::String | FieldType::Uuid | FieldType::Relation(_) => {
      "        match predicate.filter {\n          prismar::FieldFilter::String(prismar::StringFilter::Equals(value)) => Ok(value),\n          _ => Err(prismar::RuntimeError::InvalidFilter(\"expected string equality on primary key\".to_owned())),\n        }\n".to_owned()
    }
    FieldType::Int => {
      "        match predicate.filter {\n          prismar::FieldFilter::Number(prismar::NumberFilter::Equals(value)) => Ok(value as i32),\n          _ => Err(prismar::RuntimeError::InvalidFilter(\"expected numeric equality on primary key\".to_owned())),\n        }\n".to_owned()
    }
    FieldType::BigInt => {
      "        match predicate.filter {\n          prismar::FieldFilter::Number(prismar::NumberFilter::Equals(value)) => Ok(value as i64),\n          _ => Err(prismar::RuntimeError::InvalidFilter(\"expected numeric equality on primary key\".to_owned())),\n        }\n".to_owned()
    }
    FieldType::Boolean => {
      "        match predicate.filter {\n          prismar::FieldFilter::Bool(prismar::BoolFilter::Equals(value)) => Ok(value),\n          _ => Err(prismar::RuntimeError::InvalidFilter(\"expected boolean equality on primary key\".to_owned())),\n        }\n".to_owned()
    }
    FieldType::DateTime => {
      "        match predicate.filter {\n          prismar::FieldFilter::DateTime(prismar::DateTimeFilter::Equals(value)) => Ok(value),\n          _ => Err(prismar::RuntimeError::InvalidFilter(\"expected datetime equality on primary key\".to_owned())),\n        }\n".to_owned()
    }
    FieldType::Float | FieldType::Decimal => {
      "        match predicate.filter {\n          prismar::FieldFilter::Number(prismar::NumberFilter::Equals(value)) => Ok(value),\n          _ => Err(prismar::RuntimeError::InvalidFilter(\"expected numeric equality on primary key\".to_owned())),\n        }\n".to_owned()
    }
    FieldType::Json | FieldType::Bytes => {
      "        Err(prismar::RuntimeError::InvalidFilter(\"unsupported primary key type for generic filter resolution\".to_owned()))\n".to_owned()
    }
  }
}
