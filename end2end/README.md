# End-to-end fixtures

This folder contains runnable Prismar scenarios.

## Included case

- `cargo_namespace/`
  - relation-driven generation (`Cargo` belongs to `Namespace`)
  - custom generator derives (`utoipa::ToSchema`)
  - generated Diesel `schema.rs`
  - generated SQL migrations
  - runnable Rust fixture using `prismar::connection_pool()` and `prismar::with_connection()`
  - Prisma-style JSON query example

## Run

From the project root:

```bash
sh end2end/run.sh
```

## What it exercises

- `prismar generate`
- `prismar validate`
- `prismar migrate diff`
- `prismar migrate dev`
- `prismar migrate deploy`
- querying generated models from a small Rust application
- generator block parsing
- relation metadata to Diesel `belongs_to(...)`
- generated `schema.rs`
- generated `migration.sql`
- Prisma-like JSON query payloads such as:

```json
{
  "where": {
    "key": "cargo.system.api"
  }
}
```

## Database URL handling

The fixture schema only keeps the datasource `provider`.

Connection strings are passed directly at runtime:

- CLI migrations use `--database-url`
- the Rust fixture uses `prismar::connection_pool()`
