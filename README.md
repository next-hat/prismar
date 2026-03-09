# Prismar

Prismar provides three pieces that work together:

- a lightweight Prisma schema parser
- SQL migration diff/deploy tooling
- generated Diesel-ready Rust models plus Prisma-style filter input support

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

For each model it generates:

- `MyModelDb`
- `MyModelPartial`
- `MyModelUpdate`
- `type MyModelCreate = MyModelPartial`

The generated structs include Diesel derives using the `prismar::diesel` re-export.

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
    "id": "idvalue"
  }
}
```

Usage:

```rust
let input: prismar::PrismaReadManyInput = serde_json::from_value(payload)?;
let args = input.to_args()?;
let filter = args.where.unwrap();
```

Supported first-pass operators include:

- direct scalar equality
- `AND`, `OR`, `NOT`
- `equals`, `not`
- `in`, `notIn`
- `gt`, `gte`, `lt`, `lte`
- `contains`, `notContains`, `startsWith`, `endsWith`
- relation operators `some`, `every`, `none`, `is`, `isNot`

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

- keep only the datasource `provider` in the schema
- pass the database URL to CLI migrate commands with `--database-url` or `DATABASE_URL`
- pass the database URL directly to Rust code with `prismar::connection_pool()`

This avoids editor errors from newer Prisma tooling while keeping Prismar runtime and migration usage explicit.
