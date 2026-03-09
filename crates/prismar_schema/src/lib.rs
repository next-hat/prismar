use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
  #[error("invalid schema at line {line}: {message}")]
  Invalid { line: usize, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
  pub datasource: Option<Datasource>,
  pub generators: Vec<Generator>,
  pub models: Vec<Model>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generator {
  pub name: String,
  pub provider: Option<String>,
  pub output: Option<String>,
  pub config: BTreeMap<String, GeneratorValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeneratorValue {
  String(String),
  Bool(bool),
  List(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Datasource {
  pub name: String,
  pub provider: String,
  pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
  pub name: String,
  pub mapped_name: Option<String>,
  pub fields: Vec<Field>,
  pub attributes: Vec<ModelAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
  pub name: String,
  pub r#type: FieldType,
  pub optional: bool,
  pub array: bool,
  pub attributes: Vec<FieldAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
  Int,
  BigInt,
  Float,
  Decimal,
  Boolean,
  String,
  DateTime,
  Json,
  Bytes,
  Uuid,
  Relation(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldAttribute {
  Id,
  Unique,
  UpdatedAt,
  Default(String),
  Map(String),
  Relation(String),
  Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelAttribute {
  Id(String),
  Unique(String),
  Index(String),
  Map(String),
  Other(String),
}

pub fn parse_schema(input: &str) -> Result<Schema, SchemaError> {
  let lines = normalize_lines(input);
  let mut index = 0usize;
  let mut datasource = None;
  let mut generators = Vec::new();
  let mut models = Vec::new();

  while index < lines.len() {
    let line = lines[index].trim();
    if line.is_empty() {
      index += 1;
      continue;
    }

    if line.starts_with("generator ") {
      let (block, next) = collect_block(&lines, index)?;
      generators.push(parse_generator(&block, index + 1)?);
      index = next;
      continue;
    }

    if line.starts_with("datasource ") {
      let (block, next) = collect_block(&lines, index)?;
      datasource = Some(parse_datasource(&block, index + 1)?);
      index = next;
      continue;
    }

    if line.starts_with("model ") {
      let (block, next) = collect_block(&lines, index)?;
      models.push(parse_model(&block, index + 1)?);
      index = next;
      continue;
    }

    index += 1;
  }

  Ok(Schema {
    datasource,
    generators,
    models,
  })
}

pub fn find_generator<'a>(schema: &'a Schema, provider: &str) -> Option<&'a Generator> {
  schema
    .generators
    .iter()
    .find(|generator| generator.provider.as_deref() == Some(provider))
}

fn normalize_lines(input: &str) -> Vec<String> {
  input
    .lines()
    .map(|raw| {
      let mut in_quotes = false;
      let mut out = String::new();
      let chars: Vec<char> = raw.chars().collect();
      let mut i = 0usize;
      while i < chars.len() {
        if chars[i] == '"' {
          in_quotes = !in_quotes;
          out.push(chars[i]);
          i += 1;
          continue;
        }
        if !in_quotes && i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
          break;
        }
        out.push(chars[i]);
        i += 1;
      }
      out.trim_end().to_string()
    })
    .collect()
}

fn collect_block(lines: &[String], start: usize) -> Result<(Vec<String>, usize), SchemaError> {
  let mut block = Vec::new();
  let mut brace_depth = 0i32;
  let mut found_open = false;
  let mut index = start;

  while index < lines.len() {
    let line = lines[index].trim();
    if line.contains('{') {
      brace_depth += line.matches('{').count() as i32;
      found_open = true;
    }
    if line.contains('}') {
      brace_depth -= line.matches('}').count() as i32;
    }

    block.push(lines[index].clone());
    index += 1;

    if found_open && brace_depth == 0 {
      return Ok((block, index));
    }
  }

  Err(SchemaError::Invalid {
    line: start + 1,
    message: "unterminated block".to_owned(),
  })
}

fn parse_datasource(block: &[String], start_line: usize) -> Result<Datasource, SchemaError> {
  let header = block
    .first()
    .ok_or_else(|| SchemaError::Invalid {
      line: start_line,
      message: "empty datasource block".to_owned(),
    })?
    .trim();

  let name = header
    .strip_prefix("datasource ")
    .and_then(|s| s.split_whitespace().next())
    .ok_or_else(|| SchemaError::Invalid {
      line: start_line,
      message: "invalid datasource declaration".to_owned(),
    })?
    .to_owned();

  let mut provider = None;
  let mut url = None;
  for (offset, raw) in block.iter().enumerate().skip(1) {
    let line = raw.trim();
    if line.starts_with("provider") {
      provider = extract_quoted(line);
      if provider.is_none() {
        return Err(SchemaError::Invalid {
          line: start_line + offset,
          message: "invalid provider value".to_owned(),
        });
      }
      continue;
    }

    if line.starts_with("url") {
      url = parse_assignment_value(line).or_else(|| extract_quoted(line));
      if url.is_none() {
        return Err(SchemaError::Invalid {
          line: start_line + offset,
          message: "invalid url value".to_owned(),
        });
      }
    }
  }

  Ok(Datasource {
    name,
    provider: provider.unwrap_or_else(|| "postgresql".to_owned()),
    url,
  })
}

fn parse_generator(block: &[String], start_line: usize) -> Result<Generator, SchemaError> {
  let header = block
    .first()
    .ok_or_else(|| SchemaError::Invalid {
      line: start_line,
      message: "empty generator block".to_owned(),
    })?
    .trim();

  let name = header
    .strip_prefix("generator ")
    .and_then(|s| s.split_whitespace().next())
    .ok_or_else(|| SchemaError::Invalid {
      line: start_line,
      message: "invalid generator declaration".to_owned(),
    })?
    .to_owned();

  let mut provider = None;
  let mut output = None;
  let mut config = BTreeMap::new();

  for (offset, raw) in block.iter().enumerate().skip(1) {
    let line = raw.trim();
    if line.is_empty() || line == "}" {
      continue;
    }

    let Some((key, value)) = parse_key_value(line) else {
      return Err(SchemaError::Invalid {
        line: start_line + offset,
        message: format!("invalid generator entry: {line}"),
      });
    };

    let parsed_value = parse_generator_value(value).ok_or_else(|| SchemaError::Invalid {
      line: start_line + offset,
      message: format!("invalid generator value for {key}"),
    })?;

    match key {
      "provider" => {
        if let GeneratorValue::String(value) = &parsed_value {
          provider = Some(value.clone());
        }
      }
      "output" => {
        if let GeneratorValue::String(value) = &parsed_value {
          output = Some(value.clone());
        }
      }
      _ => {}
    }

    config.insert(key.to_owned(), parsed_value);
  }

  Ok(Generator {
    name,
    provider,
    output,
    config,
  })
}

fn parse_model(block: &[String], start_line: usize) -> Result<Model, SchemaError> {
  let header = block
    .first()
    .ok_or_else(|| SchemaError::Invalid {
      line: start_line,
      message: "empty model block".to_owned(),
    })?
    .trim();

  let name = header
    .strip_prefix("model ")
    .and_then(|s| s.split_whitespace().next())
    .ok_or_else(|| SchemaError::Invalid {
      line: start_line,
      message: "invalid model declaration".to_owned(),
    })?
    .to_owned();

  let mut fields = Vec::new();
  let mut attributes = Vec::new();
  let mut mapped_name = None;

  for (offset, raw) in block.iter().enumerate().skip(1) {
    let line = raw.trim();
    if line.is_empty() || line == "}" {
      continue;
    }

    if line.starts_with("@@") {
      let attr = parse_model_attribute(line);
      if let ModelAttribute::Map(value) = &attr {
        mapped_name = Some(value.clone());
      }
      attributes.push(attr);
      continue;
    }

    fields.push(parse_field(line, start_line + offset)?);
  }

  Ok(Model {
    name,
    mapped_name,
    fields,
    attributes,
  })
}

fn parse_field(line: &str, line_number: usize) -> Result<Field, SchemaError> {
  let parts = split_field_parts(line);
  if parts.len() < 2 {
    return Err(SchemaError::Invalid {
      line: line_number,
      message: format!("invalid field declaration: {line}"),
    });
  }

  let name = parts[0].to_owned();
  let mut raw_type = parts[1].to_owned();
  let optional = raw_type.ends_with('?');
  if optional {
    raw_type.pop();
  }
  let array = raw_type.ends_with("[]");
  if array {
    raw_type.truncate(raw_type.len() - 2);
  }

  let r#type = map_field_type(raw_type.as_str());
  let attributes = extract_field_attributes(line)
    .iter()
    .map(|token| parse_field_attribute(token))
    .collect();

  Ok(Field {
    name,
    r#type,
    optional,
    array,
    attributes,
  })
}

fn extract_field_attributes(line: &str) -> Vec<String> {
  let mut attributes = Vec::new();
  let mut current = String::new();
  let mut in_attribute = false;
  let mut paren_depth = 0i32;
  let mut in_quotes = false;

  for ch in line.chars() {
    if !in_attribute {
      if ch == '@' {
        in_attribute = true;
        current.push(ch);
      }
      continue;
    }

    match ch {
      '"' => {
        in_quotes = !in_quotes;
        current.push(ch);
      }
      '(' if !in_quotes => {
        paren_depth += 1;
        current.push(ch);
      }
      ')' if !in_quotes => {
        paren_depth -= 1;
        current.push(ch);
      }
      ch if ch.is_whitespace() && !in_quotes && paren_depth == 0 => {
        attributes.push(current.trim().to_owned());
        current.clear();
        in_attribute = false;
      }
      _ => current.push(ch),
    }
  }

  if in_attribute && !current.trim().is_empty() {
    attributes.push(current.trim().to_owned());
  }

  attributes
}

fn split_field_parts(line: &str) -> Vec<String> {
  let mut parts = Vec::new();
  let mut current = String::new();
  let mut paren_depth = 0i32;
  let mut in_quotes = false;

  for ch in line.chars() {
    match ch {
      '"' => {
        in_quotes = !in_quotes;
        current.push(ch);
      }
      '(' if !in_quotes => {
        paren_depth += 1;
        current.push(ch);
      }
      ')' if !in_quotes => {
        paren_depth -= 1;
        current.push(ch);
      }
      ch if ch.is_whitespace() && !in_quotes && paren_depth == 0 => {
        if !current.is_empty() {
          parts.push(current.trim().to_owned());
          current.clear();
        }
      }
      _ => current.push(ch),
    }
  }

  if !current.trim().is_empty() {
    parts.push(current.trim().to_owned());
  }

  parts
}

fn map_field_type(name: &str) -> FieldType {
  match name {
    "Int" => FieldType::Int,
    "BigInt" => FieldType::BigInt,
    "Float" => FieldType::Float,
    "Decimal" => FieldType::Decimal,
    "Boolean" => FieldType::Boolean,
    "String" => FieldType::String,
    "DateTime" => FieldType::DateTime,
    "Json" => FieldType::Json,
    "Bytes" => FieldType::Bytes,
    "Uuid" => FieldType::Uuid,
    other => FieldType::Relation(other.to_owned()),
  }
}

fn parse_field_attribute(token: &str) -> FieldAttribute {
  if token == "@id" {
    FieldAttribute::Id
  } else if token == "@unique" {
    FieldAttribute::Unique
  } else if token == "@updatedAt" {
    FieldAttribute::UpdatedAt
  } else if token.starts_with("@default(") {
    FieldAttribute::Default(trim_wrapped(token, "@default(", ")"))
  } else if token.starts_with("@map(") {
    FieldAttribute::Map(trim_wrapped(token, "@map(", ")").trim_matches('"').to_owned())
  } else if token.starts_with("@relation(") {
    FieldAttribute::Relation(trim_wrapped(token, "@relation(", ")"))
  } else {
    FieldAttribute::Other(token.to_owned())
  }
}

fn parse_model_attribute(line: &str) -> ModelAttribute {
  if line.starts_with("@@id") {
    ModelAttribute::Id(trim_wrapped(line, "@@id(", ")"))
  } else if line.starts_with("@@unique") {
    ModelAttribute::Unique(trim_wrapped(line, "@@unique(", ")"))
  } else if line.starts_with("@@index") {
    ModelAttribute::Index(trim_wrapped(line, "@@index(", ")"))
  } else if line.starts_with("@@map") {
    ModelAttribute::Map(
      trim_wrapped(line, "@@map(", ")")
        .trim_matches('"')
        .to_owned(),
    )
  } else {
    ModelAttribute::Other(line.to_owned())
  }
}

fn extract_quoted(line: &str) -> Option<String> {
  let start = line.find('"')?;
  let remaining = &line[start + 1..];
  let end = remaining.find('"')?;
  Some(remaining[..end].to_owned())
}

fn trim_wrapped(input: &str, prefix: &str, suffix: &str) -> String {
  let trimmed = input.trim();
  let trimmed = trimmed.strip_prefix(prefix).unwrap_or(trimmed);
  let trimmed = trimmed.strip_suffix(suffix).unwrap_or(trimmed);
  trimmed.trim().to_owned()
}

fn parse_assignment_value(line: &str) -> Option<String> {
  let mut parts = line.splitn(2, '=');
  let _left = parts.next()?;
  let right = parts.next()?.trim();
  if right.is_empty() {
    return None;
  }
  Some(right.trim_matches('"').to_owned())
}

fn parse_key_value(line: &str) -> Option<(&str, &str)> {
  let mut parts = line.splitn(2, '=');
  let key = parts.next()?.trim();
  let value = parts.next()?.trim();
  if key.is_empty() || value.is_empty() {
    return None;
  }
  Some((key, value))
}

fn parse_generator_value(input: &str) -> Option<GeneratorValue> {
  let trimmed = input.trim();
  if trimmed.eq_ignore_ascii_case("true") {
    return Some(GeneratorValue::Bool(true));
  }
  if trimmed.eq_ignore_ascii_case("false") {
    return Some(GeneratorValue::Bool(false));
  }
  if trimmed.starts_with('[') && trimmed.ends_with(']') {
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut values = Vec::new();
    for value in inner.split(',').map(str::trim).filter(|value| !value.is_empty()) {
      values.push(value.trim_matches('"').to_owned());
    }
    return Some(GeneratorValue::List(values));
  }
  Some(GeneratorValue::String(trimmed.trim_matches('"').to_owned()))
}
