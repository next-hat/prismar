use prismar_schema::{FieldAttribute, parse_schema};

#[test]
fn parses_prisma_schema_models_and_datasource() {
  let schema = r#"
  generator client {
    provider = "prisma-client"
  }

  generator prismar {
    provider = "prismar-cli"
    output = "./src/generated"
    db_derives = ["Debug", "utoipa::ToSchema"]
    generate_schema_rs = true
  }

  datasource db {
    provider = "sqlite"
    url = env("DATABASE_URL")
  }

  model Article {
    id          Int      @id @default(autoincrement())
    title       String
    description String?
    tags        Json     @default("[]")
    createdAt   DateTime @default(now())
    updatedAt   DateTime @updatedAt
    @@map("articles")
  }
  "#;

  let parsed = parse_schema(schema).expect("schema should parse");
  let datasource = parsed.datasource.expect("has datasource");
  assert_eq!(datasource.provider, "sqlite");
  assert_eq!(datasource.url.as_deref(), Some("env(\"DATABASE_URL\")"));
  assert_eq!(parsed.generators.len(), 2);
  let generator = parsed
    .generators
    .iter()
    .find(|generator| generator.provider.as_deref() == Some("prismar-cli"))
    .expect("prismar generator");
  assert_eq!(generator.output.as_deref(), Some("./src/generated"));
  assert_eq!(parsed.models.len(), 1);

  let article = &parsed.models[0];
  assert_eq!(article.name, "Article");
  assert_eq!(article.mapped_name.as_deref(), Some("articles"));
  assert!(article.fields.iter().any(|field| field.name == "tags"));

  let id_field = article
    .fields
    .iter()
    .find(|field| field.name == "id")
    .expect("id field");
  assert!(id_field.attributes.iter().any(|attr| {
    matches!(attr, FieldAttribute::Default(value) if value == "autoincrement()")
  }));
}

#[test]
fn preserves_nested_default_function_calls() {
  let schema = r#"
  datasource db {
    provider = "sqlite"
  }

  model Cargo {
    key String @id
    specKey String @default(uuid())
  }
  "#;

  let parsed = parse_schema(schema).expect("schema should parse");
  let field = parsed.models[0]
    .fields
    .iter()
    .find(|field| field.name == "specKey")
    .expect("specKey field");
  assert!(field.attributes.iter().any(|attr| {
    matches!(attr, FieldAttribute::Default(value) if value == "uuid()")
  }));
}

#[test]
fn parses_uuid_default_from_end2end_fixture() {
  let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../end2end/cargo_namespace/schema.prisma");
  let schema = std::fs::read_to_string(schema_path).expect("read e2e schema");
  let parsed = parse_schema(&schema).expect("schema should parse");
  let cargo = parsed
    .models
    .iter()
    .find(|model| model.name == "Cargo")
    .expect("cargo model");
  let spec_key = cargo
    .fields
    .iter()
    .find(|field| field.name == "spec_key")
    .expect("spec_key field");

  assert!(spec_key.attributes.iter().any(|attr| {
    matches!(attr, FieldAttribute::Default(value) if value == "uuid()")
  }));
}
