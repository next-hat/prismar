# Integration databases

Start PostgreSQL + MySQL for local migration test runs:

```bash
docker compose -f tests/docker-compose.databases.yml up -d
```

Stop and remove containers:

```bash
docker compose -f tests/docker-compose.databases.yml down -v
```

Suggested URLs:

- PostgreSQL: `postgresql://prismar:prismar@127.0.0.1:5432/prismar`
- MySQL: `mysql://prismar:prismar@127.0.0.1:3306/prismar`

Examples:

```bash
cargo run -p prismar-cli -- migrate status \
  --schema schema.prisma \
  --provider postgresql \
  --database-url postgresql://prismar:prismar@127.0.0.1:5432/prismar

cargo run -p prismar-cli -- migrate drift \
  --schema schema.prisma \
  --provider mysql \
  --database-url mysql://prismar:prismar@127.0.0.1:3306/prismar
```
