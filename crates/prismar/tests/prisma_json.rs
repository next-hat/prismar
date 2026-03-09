use prismar::{PrismaReadManyInput, PrismaWhereInput, SqlBackend};

#[test]
fn parses_simple_prisma_style_where_json() {
  let value = serde_json::json!({
    "where": {
      "id": "idvalue"
    }
  });

  let input: PrismaReadManyInput = serde_json::from_value(value).unwrap();
  let args = input.to_args().unwrap();
  let rendered = args.r#where.unwrap().render(SqlBackend::Postgres);
  assert!(rendered.sql.contains("id = $1"));
}

#[test]
fn parses_operator_and_relation_json() {
  let value = serde_json::json!({
    "OR": [
      { "name": { "contains": "api" } },
      { "namespace": { "is": { "name": "system" } } }
    ]
  });

  let input: PrismaWhereInput = serde_json::from_value(value).unwrap();
  let rendered = input.to_filter().unwrap().render(SqlBackend::Postgres);
  assert!(rendered.sql.contains("OR"));
  assert!(rendered.sql.contains("name LIKE"));
  assert!(rendered.sql.contains("EXISTS (SELECT 1 WHERE"));
}
