mod json_query;
mod runtime;
mod sql;

pub use diesel;
pub use diesel_migrations::{
  EmbeddedMigrations, MigrationHarness, embed_migrations,
};
pub use json_query::{
  PrismaReadManyInput, PrismaWhereInput, parse_model_filter,
};
pub use runtime::{
  PrismaClient, Prismar, Provider, RuntimeError, connection_pool,
  with_connection, with_raw_query,
};
pub use sql::{RenderedFilter, SqlBackend};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[cfg(feature = "derive")]
pub use prismar_derive::PrismarModel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ScalarValue {
  Null,
  Bool(bool),
  Number(f64),
  String(String),
  Json(serde_json::Value),
  Uuid(uuid::Uuid),
  DateTime(NaiveDateTime),
}

impl From<String> for ScalarValue {
  fn from(value: String) -> Self {
    Self::String(value)
  }
}

impl From<&str> for ScalarValue {
  fn from(value: &str) -> Self {
    Self::String(value.to_owned())
  }
}

impl From<bool> for ScalarValue {
  fn from(value: bool) -> Self {
    Self::Bool(value)
  }
}

impl From<serde_json::Value> for ScalarValue {
  fn from(value: serde_json::Value) -> Self {
    Self::Json(value)
  }
}

impl From<uuid::Uuid> for ScalarValue {
  fn from(value: uuid::Uuid) -> Self {
    Self::Uuid(value)
  }
}

impl From<NaiveDateTime> for ScalarValue {
  fn from(value: NaiveDateTime) -> Self {
    Self::DateTime(value)
  }
}

macro_rules! impl_number_value {
  ($($ty:ty),* $(,)?) => {
    $(
      impl From<$ty> for ScalarValue {
        fn from(value: $ty) -> Self {
          Self::Number(value as f64)
        }
      }
    )*
  };
}

impl_number_value!(
  i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum StringFilter {
  Equals(String),
  NotEquals(String),
  Like(String),
  NotLike(String),
  Contains(String),
  NotContains(String),
  StartsWith(String),
  EndsWith(String),
  GreaterThan(String),
  GreaterThanOrEquals(String),
  LessThan(String),
  LessThanOrEquals(String),
  In(Vec<String>),
  NotIn(Vec<String>),
  IsNull,
  IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum NumberFilter {
  Equals(f64),
  NotEquals(f64),
  GreaterThan(f64),
  GreaterThanOrEquals(f64),
  LessThan(f64),
  LessThanOrEquals(f64),
  In(Vec<f64>),
  NotIn(Vec<f64>),
  IsNull,
  IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum BoolFilter {
  Equals(bool),
  NotEquals(bool),
  In(Vec<bool>),
  NotIn(Vec<bool>),
  IsNull,
  IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum UuidFilter {
  Equals(uuid::Uuid),
  NotEquals(uuid::Uuid),
  In(Vec<uuid::Uuid>),
  NotIn(Vec<uuid::Uuid>),
  IsNull,
  IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum DateTimeFilter {
  Equals(NaiveDateTime),
  NotEquals(NaiveDateTime),
  GreaterThan(NaiveDateTime),
  GreaterThanOrEquals(NaiveDateTime),
  LessThan(NaiveDateTime),
  LessThanOrEquals(NaiveDateTime),
  In(Vec<NaiveDateTime>),
  NotIn(Vec<NaiveDateTime>),
  IsNull,
  IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum JsonFilter {
  Equals(serde_json::Value),
  NotEquals(serde_json::Value),
  Contains(serde_json::Value),
  NotContains(serde_json::Value),
  HasKey(String),
  HasAnyKey(Vec<String>),
  HasEveryKey(Vec<String>),
  PathEquals {
    path: Vec<String>,
    value: serde_json::Value,
  },
  PathNotEquals {
    path: Vec<String>,
    value: serde_json::Value,
  },
  PathLike {
    path: Vec<String>,
    value: String,
  },
  PathNotLike {
    path: Vec<String>,
    value: String,
  },
  PathStartsWith {
    path: Vec<String>,
    value: String,
  },
  PathEndsWith {
    path: Vec<String>,
    value: String,
  },
  PathContains {
    path: Vec<String>,
    value: serde_json::Value,
  },
  IsNull,
  IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum FieldFilter {
  String(StringFilter),
  Number(NumberFilter),
  Bool(BoolFilter),
  Uuid(UuidFilter),
  DateTime(DateTimeFilter),
  Json(JsonFilter),
  Null,
  NotNull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum RelationFilterOp {
  Some,
  Every,
  None,
  Is,
  IsNot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Predicate {
  pub field: String,
  pub filter: FieldFilter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RelationPredicate {
  pub field: String,
  pub op: RelationFilterOp,
  pub filter: Box<ModelFilter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum Condition {
  Predicate(Predicate),
  And(Vec<ModelFilter>),
  Or(Vec<ModelFilter>),
  Not(Box<ModelFilter>),
  Relation(RelationPredicate),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModelFilter {
  pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum OrderDirection {
  Asc,
  Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OrderBy {
  pub field: String,
  pub direction: OrderDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
  pub skip: Option<usize>,
  pub take: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReadManyArgs {
  #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
  pub r#where: Option<ModelFilter>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub order_by: Option<Vec<OrderBy>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReadUniqueArgs {
  #[serde(rename = "where")]
  pub r#where: ModelFilter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CountArgs {
  #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
  pub r#where: Option<ModelFilter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateArgs {
  pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateManyArgs {
  pub data: Vec<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub skip_duplicates: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct UpdateArgs {
  #[serde(rename = "where")]
  pub r#where: ModelFilter,
  pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct UpdateManyArgs {
  #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
  pub r#where: Option<ModelFilter>,
  pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeleteArgs {
  #[serde(rename = "where")]
  pub r#where: ModelFilter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeleteManyArgs {
  #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
  pub r#where: Option<ModelFilter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct IdSelector {
  pub field: String,
  pub value: ScalarValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReadByIdArgs {
  pub id: IdSelector,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeleteByIdArgs {
  pub id: IdSelector,
}

#[derive(
  Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BatchPayload {
  pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RawSqlQuery {
  pub sql: String,
  pub params: Vec<ScalarValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawSqlBuilder {
  backend: SqlBackend,
  base: String,
  where_filter: Option<ModelFilter>,
  order_by: Vec<OrderBy>,
  limit: Option<usize>,
  offset: Option<usize>,
}

impl ModelFilter {
  pub fn empty() -> Self {
    Self::default()
  }

  pub fn predicate(
    mut self,
    field: impl Into<String>,
    filter: FieldFilter,
  ) -> Self {
    self.conditions.push(Condition::Predicate(Predicate {
      field: field.into(),
      filter,
    }));
    self
  }

  pub fn and_group(mut self, filters: Vec<ModelFilter>) -> Self {
    if !filters.is_empty() {
      self.conditions.push(Condition::And(filters));
    }
    self
  }

  pub fn or_group(mut self, filters: Vec<ModelFilter>) -> Self {
    if !filters.is_empty() {
      self.conditions.push(Condition::Or(filters));
    }
    self
  }

  pub fn not_group(mut self, filter: ModelFilter) -> Self {
    self.conditions.push(Condition::Not(Box::new(filter)));
    self
  }

  pub fn relation(
    mut self,
    field: impl Into<String>,
    op: RelationFilterOp,
    filter: ModelFilter,
  ) -> Self {
    self.conditions.push(Condition::Relation(RelationPredicate {
      field: field.into(),
      op,
      filter: Box::new(filter),
    }));
    self
  }

  pub fn render(&self, backend: SqlBackend) -> RenderedFilter {
    sql::render_model_filter(self, backend)
  }

  pub fn is_empty(&self) -> bool {
    self.conditions.is_empty()
  }

  pub fn read_by_id(
    field: impl Into<String>,
    value: impl IntoIdFilter,
  ) -> Self {
    Self::empty().predicate(field, value.into_field_filter())
  }

  pub fn delete_by_id(
    field: impl Into<String>,
    value: impl IntoIdFilter,
  ) -> DeleteArgs {
    DeleteArgs {
      r#where: Self::read_by_id(field, value),
    }
  }
}

impl IdSelector {
  pub fn new(field: impl Into<String>, value: impl Into<ScalarValue>) -> Self {
    Self {
      field: field.into(),
      value: value.into(),
    }
  }

  pub fn to_filter(&self) -> Result<ModelFilter, String> {
    if !is_safe_identifier(self.field.as_str()) {
      return Err("Invalid id selector field".to_owned());
    }
    Ok(ModelFilter::empty().predicate(
      self.field.clone(),
      scalar_to_field_filter(self.value.clone()),
    ))
  }
}

impl ReadByIdArgs {
  pub fn to_filter(&self) -> Result<ModelFilter, String> {
    self.id.to_filter()
  }
}

impl DeleteByIdArgs {
  pub fn to_filter(&self) -> Result<ModelFilter, String> {
    self.id.to_filter()
  }
}

impl RawSqlBuilder {
  pub fn new(
    backend: SqlBackend,
    base: impl Into<String>,
  ) -> Result<Self, String> {
    let base = base.into();
    if base.contains(';') {
      return Err("Raw base query must not contain ';'".to_owned());
    }
    Ok(Self {
      backend,
      base,
      where_filter: None,
      order_by: Vec::new(),
      limit: None,
      offset: None,
    })
  }

  pub fn filter(mut self, filter: ModelFilter) -> Self {
    self.where_filter = Some(filter);
    self
  }

  pub fn order_by(
    mut self,
    field: impl Into<String>,
    direction: OrderDirection,
  ) -> Result<Self, String> {
    let field = field.into();
    if !is_safe_identifier(&field) {
      return Err("Invalid order_by field".to_owned());
    }
    self.order_by.push(OrderBy { field, direction });
    Ok(self)
  }

  pub fn limit(mut self, limit: usize) -> Self {
    self.limit = Some(limit);
    self
  }

  pub fn offset(mut self, offset: usize) -> Self {
    self.offset = Some(offset);
    self
  }

  pub fn build(self) -> Result<RawSqlQuery, String> {
    let mut sql = self.base;
    let mut params = Vec::new();
    let mut index = 0usize;

    if let Some(filter) = self.where_filter {
      let rendered = filter.render(self.backend);
      if rendered.sql != "1=1" {
        sql.push_str(" WHERE ");
        sql.push_str(rendered.sql.as_str());
        params.extend(rendered.params);
        index = params.len();
      }
    }

    if !self.order_by.is_empty() {
      sql.push_str(" ORDER BY ");
      let parts = self
        .order_by
        .into_iter()
        .map(|item| {
          let direction = match item.direction {
            OrderDirection::Asc => "ASC",
            OrderDirection::Desc => "DESC",
          };
          format!("{} {}", item.field, direction)
        })
        .collect::<Vec<_>>();
      sql.push_str(parts.join(", ").as_str());
    }

    if let Some(limit) = self.limit {
      index += 1;
      match self.backend {
        SqlBackend::Postgres => {
          sql.push_str(format!(" LIMIT ${index}").as_str())
        }
        SqlBackend::MySql | SqlBackend::Sqlite => sql.push_str(" LIMIT ?"),
      }
      params.push(ScalarValue::Number(limit as f64));
    }

    if let Some(offset) = self.offset {
      index += 1;
      match self.backend {
        SqlBackend::Postgres => {
          sql.push_str(format!(" OFFSET ${index}").as_str())
        }
        SqlBackend::MySql | SqlBackend::Sqlite => sql.push_str(" OFFSET ?"),
      }
      params.push(ScalarValue::Number(offset as f64));
    }

    Ok(RawSqlQuery { sql, params })
  }
}

pub trait IntoIdFilter {
  fn into_field_filter(self) -> FieldFilter;
}

impl IntoIdFilter for String {
  fn into_field_filter(self) -> FieldFilter {
    FieldFilter::String(StringFilter::Equals(self))
  }
}

impl IntoIdFilter for &str {
  fn into_field_filter(self) -> FieldFilter {
    FieldFilter::String(StringFilter::Equals(self.to_owned()))
  }
}

impl IntoIdFilter for uuid::Uuid {
  fn into_field_filter(self) -> FieldFilter {
    FieldFilter::Uuid(UuidFilter::Equals(self))
  }
}

impl IntoIdFilter for bool {
  fn into_field_filter(self) -> FieldFilter {
    FieldFilter::Bool(BoolFilter::Equals(self))
  }
}

impl IntoIdFilter for NaiveDateTime {
  fn into_field_filter(self) -> FieldFilter {
    FieldFilter::DateTime(DateTimeFilter::Equals(self))
  }
}

macro_rules! impl_number_id_filter {
  ($($ty:ty),* $(,)?) => {
    $(
      impl IntoIdFilter for $ty {
        fn into_field_filter(self) -> FieldFilter {
          FieldFilter::Number(NumberFilter::Equals(self as f64))
        }
      }
    )*
  };
}

impl_number_id_filter!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

fn scalar_to_field_filter(value: ScalarValue) -> FieldFilter {
  match value {
    ScalarValue::Null => FieldFilter::Null,
    ScalarValue::Bool(v) => FieldFilter::Bool(BoolFilter::Equals(v)),
    ScalarValue::Number(v) => FieldFilter::Number(NumberFilter::Equals(v)),
    ScalarValue::String(v) => FieldFilter::String(StringFilter::Equals(v)),
    ScalarValue::Json(v) => FieldFilter::Json(JsonFilter::Equals(v)),
    ScalarValue::Uuid(v) => FieldFilter::Uuid(UuidFilter::Equals(v)),
    ScalarValue::DateTime(v) => {
      FieldFilter::DateTime(DateTimeFilter::Equals(v))
    }
  }
}

fn is_safe_identifier(identifier: &str) -> bool {
  let mut chars = identifier.chars();
  let Some(first) = chars.next() else {
    return false;
  };
  if !(first == '_' || first.is_ascii_alphabetic()) {
    return false;
  }
  chars.all(|ch| ch == '_' || ch == '.' || ch.is_ascii_alphanumeric())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StringOps {
  filters: Vec<StringFilter>,
}

pub trait IntoStringOpsInput {
  fn into_ops(self) -> StringOps;
}

impl StringOps {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn eq(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::Equals(value.into()));
    self
  }

  pub fn ne(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::NotEquals(value.into()));
    self
  }

  pub fn like(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::Like(value.into()));
    self
  }

  pub fn not_like(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::NotLike(value.into()));
    self
  }

  pub fn contains(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::Contains(value.into()));
    self
  }

  pub fn not_contains(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::NotContains(value.into()));
    self
  }

  pub fn starts_with(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::StartsWith(value.into()));
    self
  }

  pub fn ends_with(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::EndsWith(value.into()));
    self
  }

  pub fn in_(mut self, value: Vec<String>) -> Self {
    self.filters.push(StringFilter::In(value));
    self
  }

  pub fn not_in(mut self, value: Vec<String>) -> Self {
    self.filters.push(StringFilter::NotIn(value));
    self
  }

  pub fn gt(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::GreaterThan(value.into()));
    self
  }

  pub fn gte(mut self, value: impl Into<String>) -> Self {
    self
      .filters
      .push(StringFilter::GreaterThanOrEquals(value.into()));
    self
  }

  pub fn lt(mut self, value: impl Into<String>) -> Self {
    self.filters.push(StringFilter::LessThan(value.into()));
    self
  }

  pub fn lte(mut self, value: impl Into<String>) -> Self {
    self
      .filters
      .push(StringFilter::LessThanOrEquals(value.into()));
    self
  }

  pub fn is_null(mut self) -> Self {
    self.filters.push(StringFilter::IsNull);
    self
  }

  pub fn is_not_null(mut self) -> Self {
    self.filters.push(StringFilter::IsNotNull);
    self
  }

  pub fn into_filters(self) -> Vec<StringFilter> {
    self.filters
  }
}

impl From<StringFilter> for StringOps {
  fn from(value: StringFilter) -> Self {
    Self {
      filters: vec![value],
    }
  }
}

impl IntoStringOpsInput for StringOps {
  fn into_ops(self) -> StringOps {
    self
  }
}

impl IntoStringOpsInput for StringFilter {
  fn into_ops(self) -> StringOps {
    self.into()
  }
}

impl<F> IntoStringOpsInput for F
where
  F: FnOnce(StringOps) -> StringOps,
{
  fn into_ops(self) -> StringOps {
    self(StringOps::new())
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NumberOps {
  filters: Vec<NumberFilter>,
}

pub trait IntoNumberOpsInput {
  fn into_ops(self) -> NumberOps;
}

impl NumberOps {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn eq(mut self, value: f64) -> Self {
    self.filters.push(NumberFilter::Equals(value));
    self
  }

  pub fn ne(mut self, value: f64) -> Self {
    self.filters.push(NumberFilter::NotEquals(value));
    self
  }

  pub fn gt(mut self, value: f64) -> Self {
    self.filters.push(NumberFilter::GreaterThan(value));
    self
  }

  pub fn gte(mut self, value: f64) -> Self {
    self.filters.push(NumberFilter::GreaterThanOrEquals(value));
    self
  }

  pub fn lt(mut self, value: f64) -> Self {
    self.filters.push(NumberFilter::LessThan(value));
    self
  }

  pub fn lte(mut self, value: f64) -> Self {
    self.filters.push(NumberFilter::LessThanOrEquals(value));
    self
  }

  pub fn in_(mut self, value: Vec<f64>) -> Self {
    self.filters.push(NumberFilter::In(value));
    self
  }

  pub fn not_in(mut self, value: Vec<f64>) -> Self {
    self.filters.push(NumberFilter::NotIn(value));
    self
  }

  pub fn is_null(mut self) -> Self {
    self.filters.push(NumberFilter::IsNull);
    self
  }

  pub fn is_not_null(mut self) -> Self {
    self.filters.push(NumberFilter::IsNotNull);
    self
  }

  pub fn into_filters(self) -> Vec<NumberFilter> {
    self.filters
  }
}

impl From<NumberFilter> for NumberOps {
  fn from(value: NumberFilter) -> Self {
    Self {
      filters: vec![value],
    }
  }
}

impl IntoNumberOpsInput for NumberOps {
  fn into_ops(self) -> NumberOps {
    self
  }
}

impl IntoNumberOpsInput for NumberFilter {
  fn into_ops(self) -> NumberOps {
    self.into()
  }
}

impl<F> IntoNumberOpsInput for F
where
  F: FnOnce(NumberOps) -> NumberOps,
{
  fn into_ops(self) -> NumberOps {
    self(NumberOps::new())
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoolOps {
  filters: Vec<BoolFilter>,
}

pub trait IntoBoolOpsInput {
  fn into_ops(self) -> BoolOps;
}

impl BoolOps {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn eq(mut self, value: bool) -> Self {
    self.filters.push(BoolFilter::Equals(value));
    self
  }

  pub fn ne(mut self, value: bool) -> Self {
    self.filters.push(BoolFilter::NotEquals(value));
    self
  }

  pub fn in_(mut self, value: Vec<bool>) -> Self {
    self.filters.push(BoolFilter::In(value));
    self
  }

  pub fn not_in(mut self, value: Vec<bool>) -> Self {
    self.filters.push(BoolFilter::NotIn(value));
    self
  }

  pub fn is_null(mut self) -> Self {
    self.filters.push(BoolFilter::IsNull);
    self
  }

  pub fn is_not_null(mut self) -> Self {
    self.filters.push(BoolFilter::IsNotNull);
    self
  }

  pub fn into_filters(self) -> Vec<BoolFilter> {
    self.filters
  }
}

impl From<BoolFilter> for BoolOps {
  fn from(value: BoolFilter) -> Self {
    Self {
      filters: vec![value],
    }
  }
}

impl IntoBoolOpsInput for BoolOps {
  fn into_ops(self) -> BoolOps {
    self
  }
}

impl IntoBoolOpsInput for BoolFilter {
  fn into_ops(self) -> BoolOps {
    self.into()
  }
}

impl<F> IntoBoolOpsInput for F
where
  F: FnOnce(BoolOps) -> BoolOps,
{
  fn into_ops(self) -> BoolOps {
    self(BoolOps::new())
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UuidOps {
  filters: Vec<UuidFilter>,
}

pub trait IntoUuidOpsInput {
  fn into_ops(self) -> UuidOps;
}

impl UuidOps {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn eq(mut self, value: uuid::Uuid) -> Self {
    self.filters.push(UuidFilter::Equals(value));
    self
  }

  pub fn ne(mut self, value: uuid::Uuid) -> Self {
    self.filters.push(UuidFilter::NotEquals(value));
    self
  }

  pub fn in_(mut self, value: Vec<uuid::Uuid>) -> Self {
    self.filters.push(UuidFilter::In(value));
    self
  }

  pub fn not_in(mut self, value: Vec<uuid::Uuid>) -> Self {
    self.filters.push(UuidFilter::NotIn(value));
    self
  }

  pub fn is_null(mut self) -> Self {
    self.filters.push(UuidFilter::IsNull);
    self
  }

  pub fn is_not_null(mut self) -> Self {
    self.filters.push(UuidFilter::IsNotNull);
    self
  }

  pub fn into_filters(self) -> Vec<UuidFilter> {
    self.filters
  }
}

impl From<UuidFilter> for UuidOps {
  fn from(value: UuidFilter) -> Self {
    Self {
      filters: vec![value],
    }
  }
}

impl IntoUuidOpsInput for UuidOps {
  fn into_ops(self) -> UuidOps {
    self
  }
}

impl IntoUuidOpsInput for UuidFilter {
  fn into_ops(self) -> UuidOps {
    self.into()
  }
}

impl<F> IntoUuidOpsInput for F
where
  F: FnOnce(UuidOps) -> UuidOps,
{
  fn into_ops(self) -> UuidOps {
    self(UuidOps::new())
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DateTimeOps {
  filters: Vec<DateTimeFilter>,
}

pub trait IntoDateTimeOpsInput {
  fn into_ops(self) -> DateTimeOps;
}

impl DateTimeOps {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn eq(mut self, value: NaiveDateTime) -> Self {
    self.filters.push(DateTimeFilter::Equals(value));
    self
  }

  pub fn ne(mut self, value: NaiveDateTime) -> Self {
    self.filters.push(DateTimeFilter::NotEquals(value));
    self
  }

  pub fn gt(mut self, value: NaiveDateTime) -> Self {
    self.filters.push(DateTimeFilter::GreaterThan(value));
    self
  }

  pub fn gte(mut self, value: NaiveDateTime) -> Self {
    self
      .filters
      .push(DateTimeFilter::GreaterThanOrEquals(value));
    self
  }

  pub fn lt(mut self, value: NaiveDateTime) -> Self {
    self.filters.push(DateTimeFilter::LessThan(value));
    self
  }

  pub fn lte(mut self, value: NaiveDateTime) -> Self {
    self.filters.push(DateTimeFilter::LessThanOrEquals(value));
    self
  }

  pub fn in_(mut self, value: Vec<NaiveDateTime>) -> Self {
    self.filters.push(DateTimeFilter::In(value));
    self
  }

  pub fn not_in(mut self, value: Vec<NaiveDateTime>) -> Self {
    self.filters.push(DateTimeFilter::NotIn(value));
    self
  }

  pub fn is_null(mut self) -> Self {
    self.filters.push(DateTimeFilter::IsNull);
    self
  }

  pub fn is_not_null(mut self) -> Self {
    self.filters.push(DateTimeFilter::IsNotNull);
    self
  }

  pub fn into_filters(self) -> Vec<DateTimeFilter> {
    self.filters
  }
}

impl From<DateTimeFilter> for DateTimeOps {
  fn from(value: DateTimeFilter) -> Self {
    Self {
      filters: vec![value],
    }
  }
}

impl IntoDateTimeOpsInput for DateTimeOps {
  fn into_ops(self) -> DateTimeOps {
    self
  }
}

impl IntoDateTimeOpsInput for DateTimeFilter {
  fn into_ops(self) -> DateTimeOps {
    self.into()
  }
}

impl<F> IntoDateTimeOpsInput for F
where
  F: FnOnce(DateTimeOps) -> DateTimeOps,
{
  fn into_ops(self) -> DateTimeOps {
    self(DateTimeOps::new())
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JsonOps {
  filters: Vec<JsonFilter>,
}

pub trait IntoJsonOpsInput {
  fn into_ops(self) -> JsonOps;
}

impl JsonOps {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn equals(mut self, value: serde_json::Value) -> Self {
    self.filters.push(JsonFilter::Equals(value));
    self
  }

  pub fn not_equals(mut self, value: serde_json::Value) -> Self {
    self.filters.push(JsonFilter::NotEquals(value));
    self
  }

  pub fn contains(mut self, value: serde_json::Value) -> Self {
    self.filters.push(JsonFilter::Contains(value));
    self
  }

  pub fn not_contains(mut self, value: serde_json::Value) -> Self {
    self.filters.push(JsonFilter::NotContains(value));
    self
  }

  pub fn has_key(mut self, key: impl Into<String>) -> Self {
    self.filters.push(JsonFilter::HasKey(key.into()));
    self
  }

  pub fn path_like(
    mut self,
    path: Vec<String>,
    value: impl Into<String>,
  ) -> Self {
    self.filters.push(JsonFilter::PathLike {
      path,
      value: value.into(),
    });
    self
  }

  pub fn path_not_like(
    mut self,
    path: Vec<String>,
    value: impl Into<String>,
  ) -> Self {
    self.filters.push(JsonFilter::PathNotLike {
      path,
      value: value.into(),
    });
    self
  }

  pub fn is_null(mut self) -> Self {
    self.filters.push(JsonFilter::IsNull);
    self
  }

  pub fn is_not_null(mut self) -> Self {
    self.filters.push(JsonFilter::IsNotNull);
    self
  }

  pub fn into_filters(self) -> Vec<JsonFilter> {
    self.filters
  }
}

impl From<JsonFilter> for JsonOps {
  fn from(value: JsonFilter) -> Self {
    Self {
      filters: vec![value],
    }
  }
}

impl IntoJsonOpsInput for JsonOps {
  fn into_ops(self) -> JsonOps {
    self
  }
}

impl IntoJsonOpsInput for JsonFilter {
  fn into_ops(self) -> JsonOps {
    self.into()
  }
}

impl<F> IntoJsonOpsInput for F
where
  F: FnOnce(JsonOps) -> JsonOps,
{
  fn into_ops(self) -> JsonOps {
    self(JsonOps::new())
  }
}

pub trait TypedFilter {
  fn into_model_filter(self) -> ModelFilter;
}

impl TypedFilter for ModelFilter {
  fn into_model_filter(self) -> ModelFilter {
    self
  }
}

pub fn parse_uuid(value: impl AsRef<str>) -> Result<uuid::Uuid, uuid::Error> {
  uuid::Uuid::parse_str(value.as_ref())
}

pub trait PrismaCreateData: Send + 'static {
  type Model: PrismaModel<Create = Self>;

  fn apply_defaults(&mut self) {}
}

pub trait PrismaUpdateData: Clone + Send + 'static {
  type Model: PrismaModel<Update = Self>;
}

#[allow(async_fn_in_trait)]
pub trait PrismaModel: Sized + Send + 'static {
  type Create: Clone + PrismaCreateData<Model = Self>;
  type Update: Clone + PrismaUpdateData<Model = Self>;
  type Id: Clone + Send + 'static;

  fn primary_key_field() -> &'static str;

  fn id(&self) -> Self::Id;

  fn id_from_filter(filter: ModelFilter) -> Result<Self::Id, RuntimeError>;

  fn id_from_create(data: &Self::Create) -> Option<Self::Id>;

  fn matches_filter(&self, filter: &ModelFilter) -> Result<bool, RuntimeError>;

  async fn create(
    client: &PrismaClient,
    data: Self::Create,
  ) -> Result<usize, RuntimeError>;

  async fn find_many(
    client: &PrismaClient,
    filter: Option<ModelFilter>,
  ) -> Result<Vec<Self>, RuntimeError>;

  async fn find_by_id(
    client: &PrismaClient,
    id: &Self::Id,
  ) -> Result<Option<Self>, RuntimeError>;

  async fn update_by_id(
    client: &PrismaClient,
    id: &Self::Id,
    data: Self::Update,
  ) -> Result<usize, RuntimeError>;

  async fn delete_by_id(
    client: &PrismaClient,
    id: &Self::Id,
  ) -> Result<usize, RuntimeError>;
}

pub fn evaluate_string_field(
  value: Option<&str>,
  filter: &FieldFilter,
) -> Result<bool, RuntimeError> {
  match filter {
    FieldFilter::Null => Ok(value.is_none()),
    FieldFilter::NotNull => Ok(value.is_some()),
    FieldFilter::String(filter) => {
      let Some(value) = value else {
        return Ok(matches!(filter, StringFilter::IsNull));
      };
      Ok(match filter {
        StringFilter::Equals(expected) => value == expected,
        StringFilter::NotEquals(expected) => value != expected,
        StringFilter::Like(pattern) => like_matches(value, pattern),
        StringFilter::NotLike(pattern) => !like_matches(value, pattern),
        StringFilter::Contains(expected) => value.contains(expected),
        StringFilter::NotContains(expected) => !value.contains(expected),
        StringFilter::StartsWith(expected) => value.starts_with(expected),
        StringFilter::EndsWith(expected) => value.ends_with(expected),
        StringFilter::GreaterThan(expected) => value > expected.as_str(),
        StringFilter::GreaterThanOrEquals(expected) => {
          value >= expected.as_str()
        }
        StringFilter::LessThan(expected) => value < expected.as_str(),
        StringFilter::LessThanOrEquals(expected) => value <= expected.as_str(),
        StringFilter::In(values) => {
          values.iter().any(|expected| value == expected)
        }
        StringFilter::NotIn(values) => {
          values.iter().all(|expected| value != expected)
        }
        StringFilter::IsNull => false,
        StringFilter::IsNotNull => true,
      })
    }
    _ => Err(RuntimeError::InvalidFilter(
      "expected string filter".to_owned(),
    )),
  }
}

pub fn evaluate_number_field(
  value: Option<f64>,
  filter: &FieldFilter,
) -> Result<bool, RuntimeError> {
  match filter {
    FieldFilter::Null => Ok(value.is_none()),
    FieldFilter::NotNull => Ok(value.is_some()),
    FieldFilter::Number(filter) => {
      let Some(value) = value else {
        return Ok(matches!(filter, NumberFilter::IsNull));
      };
      Ok(match filter {
        NumberFilter::Equals(expected) => value == *expected,
        NumberFilter::NotEquals(expected) => value != *expected,
        NumberFilter::GreaterThan(expected) => value > *expected,
        NumberFilter::GreaterThanOrEquals(expected) => value >= *expected,
        NumberFilter::LessThan(expected) => value < *expected,
        NumberFilter::LessThanOrEquals(expected) => value <= *expected,
        NumberFilter::In(values) => values.contains(&value),
        NumberFilter::NotIn(values) => {
          values.iter().all(|expected| value != *expected)
        }
        NumberFilter::IsNull => false,
        NumberFilter::IsNotNull => true,
      })
    }
    _ => Err(RuntimeError::InvalidFilter(
      "expected number filter".to_owned(),
    )),
  }
}

pub fn evaluate_bool_field(
  value: Option<bool>,
  filter: &FieldFilter,
) -> Result<bool, RuntimeError> {
  match filter {
    FieldFilter::Null => Ok(value.is_none()),
    FieldFilter::NotNull => Ok(value.is_some()),
    FieldFilter::Bool(filter) => {
      let Some(value) = value else {
        return Ok(matches!(filter, BoolFilter::IsNull));
      };
      Ok(match filter {
        BoolFilter::Equals(expected) => value == *expected,
        BoolFilter::NotEquals(expected) => value != *expected,
        BoolFilter::In(values) => values.contains(&value),
        BoolFilter::NotIn(values) => {
          values.iter().all(|expected| value != *expected)
        }
        BoolFilter::IsNull => false,
        BoolFilter::IsNotNull => true,
      })
    }
    _ => Err(RuntimeError::InvalidFilter(
      "expected bool filter".to_owned(),
    )),
  }
}

pub fn evaluate_datetime_field(
  value: Option<NaiveDateTime>,
  filter: &FieldFilter,
) -> Result<bool, RuntimeError> {
  match filter {
    FieldFilter::Null => Ok(value.is_none()),
    FieldFilter::NotNull => Ok(value.is_some()),
    FieldFilter::DateTime(filter) => {
      let Some(value) = value else {
        return Ok(matches!(filter, DateTimeFilter::IsNull));
      };
      Ok(match filter {
        DateTimeFilter::Equals(expected) => value == *expected,
        DateTimeFilter::NotEquals(expected) => value != *expected,
        DateTimeFilter::GreaterThan(expected) => value > *expected,
        DateTimeFilter::GreaterThanOrEquals(expected) => value >= *expected,
        DateTimeFilter::LessThan(expected) => value < *expected,
        DateTimeFilter::LessThanOrEquals(expected) => value <= *expected,
        DateTimeFilter::In(values) => values.contains(&value),
        DateTimeFilter::NotIn(values) => {
          values.iter().all(|expected| value != *expected)
        }
        DateTimeFilter::IsNull => false,
        DateTimeFilter::IsNotNull => true,
      })
    }
    _ => Err(RuntimeError::InvalidFilter(
      "expected datetime filter".to_owned(),
    )),
  }
}

pub fn evaluate_json_field(
  value: Option<&serde_json::Value>,
  filter: &FieldFilter,
) -> Result<bool, RuntimeError> {
  match filter {
    FieldFilter::Null => Ok(value.is_none()),
    FieldFilter::NotNull => Ok(value.is_some()),
    FieldFilter::Json(filter) => {
      let Some(value) = value else {
        return Ok(matches!(filter, JsonFilter::IsNull));
      };
      Ok(match filter {
        JsonFilter::Equals(expected) => value == expected,
        JsonFilter::NotEquals(expected) => value != expected,
        JsonFilter::Contains(expected) => json_contains(value, expected),
        JsonFilter::NotContains(expected) => !json_contains(value, expected),
        JsonFilter::HasKey(key) => value.get(key).is_some(),
        JsonFilter::HasAnyKey(keys) => {
          keys.iter().any(|key| value.get(key).is_some())
        }
        JsonFilter::HasEveryKey(keys) => {
          keys.iter().all(|key| value.get(key).is_some())
        }
        JsonFilter::PathEquals {
          path,
          value: expected,
        } => json_path(value, path) == Some(expected),
        JsonFilter::PathNotEquals {
          path,
          value: expected,
        } => json_path(value, path) != Some(expected),
        JsonFilter::PathLike {
          path,
          value: expected,
        } => json_path(value, path)
          .and_then(serde_json::Value::as_str)
          .is_some_and(|current| like_matches(current, expected)),
        JsonFilter::PathNotLike {
          path,
          value: expected,
        } => json_path(value, path)
          .and_then(serde_json::Value::as_str)
          .is_some_and(|current| !like_matches(current, expected)),
        JsonFilter::PathStartsWith {
          path,
          value: expected,
        } => json_path(value, path)
          .and_then(serde_json::Value::as_str)
          .is_some_and(|current| current.starts_with(expected)),
        JsonFilter::PathEndsWith {
          path,
          value: expected,
        } => json_path(value, path)
          .and_then(serde_json::Value::as_str)
          .is_some_and(|current| current.ends_with(expected)),
        JsonFilter::PathContains {
          path,
          value: expected,
        } => json_path(value, path)
          .is_some_and(|current| json_contains(current, expected)),
        JsonFilter::IsNull => false,
        JsonFilter::IsNotNull => true,
      })
    }
    _ => Err(RuntimeError::InvalidFilter(
      "expected json filter".to_owned(),
    )),
  }
}

fn like_matches(value: &str, pattern: &str) -> bool {
  fn inner(value: &[u8], pattern: &[u8]) -> bool {
    match pattern.split_first() {
      None => value.is_empty(),
      Some((b'%', rest)) => {
        (0..=value.len()).any(|index| inner(&value[index..], rest))
      }
      Some((b'_', rest)) => !value.is_empty() && inner(&value[1..], rest),
      Some((current, rest)) => {
        value.first() == Some(current) && inner(&value[1..], rest)
      }
    }
  }

  inner(value.as_bytes(), pattern.as_bytes())
}

fn json_contains(
  current: &serde_json::Value,
  expected: &serde_json::Value,
) -> bool {
  match (current, expected) {
    (
      serde_json::Value::Object(current),
      serde_json::Value::Object(expected),
    ) => expected.iter().all(|(key, expected)| {
      current
        .get(key)
        .is_some_and(|value| json_contains(value, expected))
    }),
    (serde_json::Value::Array(current), serde_json::Value::Array(expected)) => {
      expected.iter().all(|expected_item| {
        current
          .iter()
          .any(|item| json_contains(item, expected_item))
      })
    }
    _ => current == expected,
  }
}

fn json_path<'a>(
  value: &'a serde_json::Value,
  path: &[String],
) -> Option<&'a serde_json::Value> {
  let mut current = value;
  for segment in path {
    current = current.get(segment)?;
  }
  Some(current)
}
