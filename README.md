# Prismar

[![Tests](https://github.com/next-hat/prismar/actions/workflows/tests.yml/badge.svg)](https://github.com/next-hat/prismar/actions/workflows/tests.yml)
[![Clippy](https://github.com/next-hat/prismar/actions/workflows/clippy.yml/badge.svg)](https://github.com/next-hat/prismar/actions/workflows/clippy.yml)
[![codecov](https://codecov.io/gh/next-hat/prismar/branch/master/graph/badge.svg?token=4I60HOW6HM)](https://codecov.io/gh/next-hat/prismar)


Prismar provides three pieces that work together:

- a lightweight Prisma schema parser
- SQL migration diff/deploy tooling
- generated Diesel-ready Rust models plus Prisma-style filter input support

## Quick start

Example schema:

```prisma
generator prismar {
  provider = "prismar-cli"
  output = "./generated"
  db_derives = ["utoipa::ToSchema"]
  partial_derives = ["utoipa::ToSchema"]
  update_derives = ["utoipa::ToSchema"]
  generate_json_types = true
}

model Namespace {
  name String @id
  cargos Cargo[]
  @@map("namespaces")
}

model Cargo {
  key            String    @id
  created_at     DateTime
  name           String
  spec_key       String    @default(uuid())
  status_key     String
  namespace_name String
  namespace      Namespace @relation(fields: [namespace_name], references: [name])
  @@map("cargoes")
}
```

Generate code:

```bash
cargo run -p prismar-cli -- generate --schema schema.prisma
```

Create a runtime client:

```rust
use generated::{CargoCreate, CargoDb, CargoDbFilter, CargoUpdate, NamespaceCreate};
use prismar::{PrismaClient, Provider, StringFilter};

#[path = "./generated/mod.rs"]
mod generated;

#[ntex::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = PrismaClient::new(Provider::Sqlite, "sqlite://./dev.db")?;

  let namespace = client
    .create(NamespaceCreate {
      name: Some("system".to_owned()),
    })
    .await?;

  let created = client
    .create(CargoCreate {
      key: Some("cargo.system.api".to_owned()),
      created_at: Some(chrono::Utc::now().naive_utc()),
      name: Some("api".to_owned()),
      spec_key: None,
      status_key: Some("running".to_owned()),
      namespace_name: Some("system".to_owned()),
    })
    .await?;

  let all_cargos = client.find_many::<CargoDb>(None).await?;
  let filtered = client
    .find_unique::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
    )
    .await?;

  let updated = client
    .update::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
      CargoUpdate {
        status_key: Some("updated".to_owned()),
        ..Default::default()
      },
    )
    .await?;

  let deleted = client
    .delete::<CargoDb, _>(
      CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
    )
    .await?;

  assert_eq!(all_cargos.len(), 1);
  assert_eq!(namespace.name, "system");
  assert_eq!(created.key, "cargo.system.api");
  assert!(filtered.is_some());
  assert_eq!(updated.status_key, "updated");
  assert_eq!(deleted.key, "cargo.system.api");
  Ok(())
}
```

## Workspace crates

- `crates/prismar`
  - runtime/filter library
  - Diesel re-export through `prismar::diesel`
  - Diesel migration re-export through `prismar::embed_migrations`, `prismar::EmbeddedMigrations`, `prismar::MigrationHarness`
  - Prisma-style JSON input support via `PrismaReadManyInput` and `PrismaWhereInput`
- `crates/prismar_schema`
  - parses `datasource`, `generator`, and `model` blocks from `schema.prisma`
- `crates/prismar_migrate`
  - schema diff to SQL
- `bin/prismar-cli`
  - `validate`
  - `generate`
  - `migrate diff|dev|status|deploy|drift`

## Generator block

Add a `generator prismar` block to control code generation:

```prisma
generator prismar {
  provider = "prismar-cli"
  output = "./src/generated"
  db_derives = ["utoipa::ToSchema"]
  partial_derives = ["utoipa::ToSchema"]
  update_derives = ["utoipa::ToSchema"]
  generate_json_types = true
}
```

## Commands

### Validate

```bash
cargo run -p prismar-cli -- validate --schema schema.prisma
```

### Generate

```bash
cargo run -p prismar-cli -- generate --schema schema.prisma
```

Or override the output path:

```bash
cargo run -p prismar-cli -- generate --schema schema.prisma --output ./src/generated
```

### Migrate

```bash
cargo run -p prismar-cli -- migrate diff --schema schema.prisma
cargo run -p prismar-cli -- migrate dev --schema schema.prisma --name init
cargo run -p prismar-cli -- migrate status --schema schema.prisma
cargo run -p prismar-cli -- migrate deploy --schema schema.prisma
```

## Generated output

`prismar generate` emits:

- `mod.rs`
- `schema.rs`
- one Rust module per model
- optional JSON helper aliases when `generate_json_types = true`

For each model it generates:

- `MyModelDb`
- `MyModelPartial`
- `MyModelUpdate`
- `type MyModelCreate = MyModelPartial`

The generated structs include Diesel derives using the `prismar::diesel` re-export.

Generated modules also implement the generic runtime traits used by `PrismaClient`, so queries stay model-centric instead of calling Diesel tables directly.

## Relations

Relations like:

```prisma
model Namespace {
  name String @id
  cargos Cargo[]
}

model Cargo {
  key            String    @id
  namespace_name String
  namespace      Namespace @relation(fields: [namespace_name], references: [name])
}
```

produce Diesel association metadata like:

```rust
#[diesel(belongs_to(NamespaceDb, foreign_key = namespace_name))]
```

and matching entries in generated `schema.rs`.

## Prisma-style JSON query input

Prismar can deserialize simple Prisma-like JSON payloads and convert them into `ModelFilter`.

Example payload:

```json
{
  "where": {
    "key": "cargo.system.api"
  }
}
```

Usage:

```rust
let input: prismar::PrismaReadManyInput = serde_json::from_value(payload)?;
let args = input.to_args()?;
let filter = args.where.unwrap();

let rows = client.find_many::<CargoDb>(Some(filter)).await?;
```

That `find_many()` example is valid today and executes against the loaded model rows.

Another example with logical composition:

```json
{
  "where": {
    "AND": [
      { "namespace_name": "system" },
      { "status_key": { "not": "stopped" } }
    ]
  }
}
```

And a relation-flavored example:

```json
{
  "where": {
    "namespace": {
      "is": {
        "name": "system"
      }
    }
  }
}
```

Supported first-pass operators include:

- direct scalar equality
- `AND`, `OR`, `NOT`
- `equals`, `not`
- `in`, `notIn`
- `gt`, `gte`, `lt`, `lte`
- `contains`, `notContains`, `startsWith`, `endsWith`
- relation operators `some`, `every`, `none`, `is`, `isNot`

## Querying with generated filters

Generated filter builders are usually the most ergonomic way to query from Rust.

### Find many

```rust
let cargos = client.find_many::<CargoDb>(None).await?;

let filtered = client
  .find_many::<CargoDb>(Some(
    CargoDbFilter::new()
      .key(StringFilter::Equals("cargo.system.api".to_owned()))
      .into(),
  ))
  .await?;
```

### Find unique

```rust
let cargo = client
  .find_unique::<CargoDb, _>(
    CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
  )
  .await?;
```

### Find first

```rust
let first = client
  .find_first::<CargoDb>(Some(
    CargoDbFilter::new()
      .status_key(StringFilter::Equals("running".to_owned()))
      .into(),
  ))
  .await?;
```

### Find one by id

```rust
let cargo_id = "cargo.system.api".to_owned();
let cargo = client.find_by_id::<CargoDb>(&cargo_id).await?;
```

### Create

`Create` is an alias of the generated partial type.

```rust
client
  .create(CargoCreate {
    key: Some("cargo.system.api".to_owned()),
    created_at: Some(chrono::Utc::now().naive_utc()),
    name: Some("api".to_owned()),
    spec_key: None,
    status_key: Some("running".to_owned()),
    namespace_name: Some("system".to_owned()),
  })
  .await?;
```

If a field has `@default(uuid())` and the Rust value is `None`, Prismar fills it client-side before insert.

### Create many

```rust
let batch = client
  .create_many(
    vec![
      CargoCreate {
        key: Some("cargo.system.api".to_owned()),
        created_at: Some(chrono::Utc::now().naive_utc()),
        name: Some("api".to_owned()),
        spec_key: None,
        status_key: Some("running".to_owned()),
        namespace_name: Some("system".to_owned()),
      },
      CargoCreate {
        key: Some("cargo.system.worker".to_owned()),
        created_at: Some(chrono::Utc::now().naive_utc()),
        name: Some("worker".to_owned()),
        spec_key: None,
        status_key: Some("running".to_owned()),
        namespace_name: Some("system".to_owned()),
      },
    ],
    None,
  )
  .await?;

assert_eq!(batch.count, 2);
```

### Update by id

```rust
let cargo_id = "cargo.system.api".to_owned();

client
  .update_by_id::<CargoUpdate>(
    &cargo_id,
    CargoUpdate {
      status_key: Some("updated".to_owned()),
      ..Default::default()
    },
  )
  .await?;
```

### Delete by id

```rust
let cargo_id = "cargo.system.api".to_owned();
client.delete_by_id::<CargoDb>(&cargo_id).await?;
```

### Update

```rust
let updated = client
  .update::<CargoDb, _>(
    CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
    CargoUpdate {
      status_key: Some("updated".to_owned()),
      ..Default::default()
    },
  )
  .await?;
```

### Update many

```rust
let batch = client
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

assert_eq!(batch.count, 1);
```

### Update many and return

```rust
let updated = client
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
```

### Upsert

```rust
let cargo = client
  .upsert::<CargoDb, _>(
    CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
    CargoCreate {
      key: Some("cargo.system.api".to_owned()),
      created_at: Some(chrono::Utc::now().naive_utc()),
      name: Some("api".to_owned()),
      spec_key: None,
      status_key: Some("running".to_owned()),
      namespace_name: Some("system".to_owned()),
    },
    CargoUpdate {
      status_key: Some("updated".to_owned()),
      ..Default::default()
    },
  )
  .await?;
```

### Delete

```rust
let deleted = client
  .delete::<CargoDb, _>(
    CargoDbFilter::new().key(StringFilter::Equals("cargo.system.api".to_owned())),
  )
  .await?;
```

### Delete many

```rust
let batch = client
  .delete_many::<CargoDb>(Some(
    CargoDbFilter::new()
      .namespace_name(StringFilter::Equals("system".to_owned()))
      .into(),
  ))
  .await?;
```

### Count

```rust
let count = client
  .count::<CargoDb>(Some(
    CargoDbFilter::new()
      .status_key(StringFilter::Equals("running".to_owned()))
      .into(),
  ))
  .await?;
```

## Prisma-style CRUD naming

Prismar now exposes Prisma-aligned CRUD entry points:

- `create`
- `create_many`
- `create_many_and_return`
- `find_unique`
- `find_first`
- `find_many`
- `count`
- `update`
- `update_many`
- `update_many_and_return`
- `upsert`
- `delete`
- `delete_many`

Additional convenience helpers remain available:

- `find_by_id`
- `update_by_id`
- `delete_by_id`

Current implementation notes:

- scalar filters execute against loaded model rows, so Prisma-style field operators such as `contains`, `startsWith`, `gt`, `in`, `AND`, `OR`, and `NOT` work through the generated model layer
- relation filter parsing exists, but relation-filter execution is not wired yet
- `create()` reloads the created row via the generated primary key, so the primary key must be present or client-generated in the create payload

## Runtime helpers

If you want lower-level access to a Diesel pool while keeping Prismar utilities, use `connection_pool()` and `with_connection()`:

```rust
use diesel::sqlite::SqliteConnection;
use prismar::{connection_pool, with_connection};

let pool = connection_pool::<SqliteConnection>("sqlite://./dev.db")?;

with_connection(pool, |conn| {
  diesel::sql_query("select 1").execute(conn)?;
  Ok(())
})
.await?;
```

## Diesel migration table

Migration status/deploy uses Diesel's metadata table:

- `__diesel_schema_migrations`

instead of a custom migration bookkeeping table.

## Backend features

`prismar-cli` supports backend features per SQL backend:

- `sqlite`
- `postgres`
- `mysql`

Default CLI features are:

- `sqlite`
- `postgres`

Examples:

```bash
cargo check -p prismar-cli --no-default-features --features sqlite
cargo check -p prismar-cli --no-default-features --features postgres
cargo check -p prismar-cli --no-default-features --features mysql
```

## End-to-end example

See [end2end/README.md](end2end/README.md) and the fixture in [end2end/cargo_namespace](end2end/cargo_namespace).

## Database URLs

Prismar does not require a datasource `url` in `schema.prisma`.

Recommended flow:

- keep only the datasource `provider` in the schema, or omit the datasource entirely if provider selection is fully runtime-driven
- pass the database URL to CLI migrate commands with `--database-url` or `DATABASE_URL`
- pass the database URL directly to Rust code with `PrismaClient::new(...)` or `prismar::connection_pool()`

This avoids editor errors from newer Prisma tooling while keeping Prismar runtime and migration usage explicit.
