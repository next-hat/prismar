#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")/.."

SCHEMA="end2end/cargo_namespace/schema.prisma"
OUT_DIR="end2end/cargo_namespace/generated"
MIGRATIONS_DIR="end2end/cargo_namespace/migrations"
DATABASE_URL="sqlite://end2end/cargo_namespace/dev.db"

rm -rf "$OUT_DIR"
rm -rf "$MIGRATIONS_DIR"
rm -f end2end/cargo_namespace/dev.db

cargo run -p prismar-cli -- generate --schema "$SCHEMA" --output "$OUT_DIR"
cargo run -p prismar-cli -- validate --schema "$SCHEMA"
cargo run -p prismar-cli -- migrate diff --schema "$SCHEMA" --provider sqlite
cargo run -p prismar-cli -- migrate dev --schema "$SCHEMA" --migrations-dir "$MIGRATIONS_DIR" --name init --provider sqlite
DATABASE_URL="$DATABASE_URL" cargo run --manifest-path end2end/cargo_namespace/Cargo.toml

echo "Generated files:"
find "$OUT_DIR" -maxdepth 2 -type f | sort

echo "Generated migration files:"
find "$MIGRATIONS_DIR" -maxdepth 3 -type f | sort

echo "Prisma-style JSON example:"
cat end2end/cargo_namespace/query.json
