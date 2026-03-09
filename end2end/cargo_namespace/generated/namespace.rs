#[allow(unused_imports)]
use prismar::PrismaModel;
use prismar::diesel::{ExpressionMethods, JoinOnDsl, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};

use super::CargoDb;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, prismar::PrismarModel, prismar::diesel::Queryable, prismar::diesel::Selectable, prismar::diesel::Insertable, prismar::diesel::Identifiable, utoipa::ToSchema)]
#[diesel(table_name = super::schema::namespaces)]
#[diesel(primary_key(name))]
pub struct NamespaceDb {
  pub name: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, prismar::diesel::Insertable, utoipa::ToSchema)]
#[diesel(table_name = super::schema::namespaces)]
pub struct NamespacePartial {
  pub name: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[allow(dead_code)]
pub struct NamespaceUpdate;

#[allow(dead_code)]
pub type NamespaceCreate = NamespacePartial;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NamespaceDbWithRelations {
  pub data: NamespaceDb,
  pub cargos: Vec<CargoDb>,
}

impl NamespaceDbWithRelations {
  async fn matches_filter(&self, client: &prismar::PrismaClient, filter: &prismar::ModelFilter) -> Result<bool, prismar::RuntimeError> {
    for condition in &filter.conditions {
      let matched = match condition {
        prismar::Condition::Predicate(predicate) => match predicate.field.as_str() {
          "name" => prismar::evaluate_string_field(Some(self.data.name.as_str()), &predicate.filter),
          unknown => Err(prismar::RuntimeError::InvalidFilter(format!("unknown field '{}'", unknown))),
        }?,
        prismar::Condition::And(filters) => {
          let mut all_match = true;
          for inner in filters {
            if !std::boxed::Box::pin(self.matches_filter(client, inner)).await? {
              all_match = false;
              break;
            }
          }
          all_match
        }
        prismar::Condition::Or(filters) => {
          let mut any_match = false;
          for inner in filters {
            if std::boxed::Box::pin(self.matches_filter(client, inner)).await? {
              any_match = true;
              break;
            }
          }
          any_match
        }
        prismar::Condition::Not(inner) => !std::boxed::Box::pin(self.matches_filter(client, inner)).await?,
        prismar::Condition::Relation(relation) => match relation.field.as_str() {
          "cargos" => match relation.op {
            prismar::RelationFilterOp::Some => {
              let mut matched = false;
              for related in &self.cargos {
                if std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {
                  matched = true;
                  break;
                }
              }
              Ok(matched)
            }
            prismar::RelationFilterOp::Every => {
              let mut matched = true;
              for related in &self.cargos {
                if !std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {
                  matched = false;
                  break;
                }
              }
              Ok(matched)
            }
            prismar::RelationFilterOp::None => {
              let mut matched = true;
              for related in &self.cargos {
                if std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {
                  matched = false;
                  break;
                }
              }
              Ok(matched)
            }
            prismar::RelationFilterOp::Is | prismar::RelationFilterOp::IsNot => Err(prismar::RuntimeError::InvalidFilter(format!("relation 'cargos' is to-many; use some/every/none"))),
          },
          unknown => Err(prismar::RuntimeError::InvalidFilter(format!("unknown relation '{}'", unknown))),
        }?,
      };
      if !matched {
        return Ok(false);
      }
    }
    Ok(true)
  }
}

impl NamespaceDbFilter {
  pub fn include_cargos(self) -> Self {
    self.include("cargos")
  }

  pub fn include_cargos_where<T: prismar::TypedFilter>(self, filter: T) -> Self {
    self.include_with("cargos", filter)
  }

}

impl NamespaceDb {
  pub async fn find_many_with(client: &prismar::PrismaClient, query: NamespaceDbFilter) -> Result<Vec<NamespaceDbWithRelations>, prismar::RuntimeError> {
    let filter = query.clone().build();
    let includes = query.includes().to_vec();
    if includes.len() > 1 {
      return Err(prismar::RuntimeError::InvalidFilter("multiple includes are not supported yet".to_owned()));
    }
    let mut rows: Vec<NamespaceDbWithRelations> = if let Some(include) = includes.first() {
      let include_filter = include.filter.clone();
      match include.relation.as_str() {
        "cargos" => {
          let rows: Result<Vec<(NamespaceDb, Option<CargoDb>)>, prismar::RuntimeError> = match client.provider() {
      prismar::Provider::Sqlite => {
        #[cfg(feature = "sqlite")] {
client.run_sqlite(|conn| { super::schema::namespaces::table.left_outer_join(super::schema::cargoes::table.on(super::schema::namespaces::name.eq(super::schema::cargoes::namespace_name))).select((NamespaceDb::as_select(), Option::<CargoDb>::as_select())).load::<(NamespaceDb, Option<CargoDb>)>(conn) }).await        }
        #[cfg(not(feature = "sqlite"))] { Err(prismar::RuntimeError::UnsupportedProvider("sqlite")) }
      }
      prismar::Provider::Postgres => {
        #[cfg(feature = "postgres")] {
client.run_postgres(|conn| { super::schema::namespaces::table.left_outer_join(super::schema::cargoes::table.on(super::schema::namespaces::name.eq(super::schema::cargoes::namespace_name))).select((NamespaceDb::as_select(), Option::<CargoDb>::as_select())).load::<(NamespaceDb, Option<CargoDb>)>(conn) }).await        }
        #[cfg(not(feature = "postgres"))] { Err(prismar::RuntimeError::UnsupportedProvider("postgres")) }
      }
      prismar::Provider::MySql => {
        #[cfg(feature = "mysql")] {
client.run_mysql(|conn| { super::schema::namespaces::table.left_outer_join(super::schema::cargoes::table.on(super::schema::namespaces::name.eq(super::schema::cargoes::namespace_name))).select((NamespaceDb::as_select(), Option::<CargoDb>::as_select())).load::<(NamespaceDb, Option<CargoDb>)>(conn) }).await        }
        #[cfg(not(feature = "mysql"))] { Err(prismar::RuntimeError::UnsupportedProvider("mysql")) }
      }
    };
          let include_filter = include_filter.unwrap_or_default();
          let mut grouped: Vec<NamespaceDbWithRelations> = Vec::new();
          for (data, related) in rows? {
            let related = if include_filter.is_empty() {
              related
            } else {
              match related {
                Some(related) => {
                  if std::boxed::Box::pin(related.matches_filter(client, &include_filter)).await? {
                    Some(related)
                  } else {
                    None
                  }
                }
                None => None,
              }
            };
            if let Some(existing) = grouped.iter_mut().find(|item| item.data.name == data.name) {
              if let Some(related) = related {
                existing.cargos.push(related);
              }
            } else {
              let mut item = NamespaceDbWithRelations {
                data: data.clone(),
                cargos: Vec::new(),
              };
              if let Some(related) = related {
                item.cargos.push(related);
              }
              grouped.push(item);
            }
          }
          grouped
        }
        unknown => return Err(prismar::RuntimeError::InvalidFilter(format!("unknown include '{}'", unknown))),
      }
    } else {
      NamespaceDb::find_many(client, None).await?.into_iter().map(|data| NamespaceDbWithRelations {
        cargos: Vec::new(),
        data,
      }).collect::<Vec<_>>()
    };
    if !filter.is_empty() {
      let mut filtered = Vec::new();
      for row in rows.drain(..) {
        if row.matches_filter(client, &filter).await? {
          filtered.push(row);
        }
      }
      rows = filtered;
    }
    Ok(rows)
  }

  pub async fn find_first_with(client: &prismar::PrismaClient, query: NamespaceDbFilter) -> Result<Option<NamespaceDbWithRelations>, prismar::RuntimeError> {
    Ok(Self::find_many_with(client, query).await?.into_iter().next())
  }

  pub async fn find_unique_with(client: &prismar::PrismaClient, query: NamespaceDbFilter) -> Result<Option<NamespaceDbWithRelations>, prismar::RuntimeError> {
    let mut rows = Self::find_many_with(client, query).await?;
    if rows.len() > 1 {
      return Err(prismar::RuntimeError::NonUniqueResult("NamespaceDb".to_owned()));
    }
    Ok(rows.pop())
  }
}

impl NamespaceDb {
  pub async fn cargos(&self, client: &prismar::PrismaClient, filter: Option<prismar::ModelFilter>) -> Result<Vec<CargoDb>, prismar::RuntimeError> {
    let value = self.name.clone();
    let rows = match client.provider() {
      prismar::Provider::Sqlite => {
        #[cfg(feature = "sqlite")] {
      client.run_sqlite(move |conn| { super::schema::cargoes::table.filter(super::schema::cargoes::namespace_name.eq(value)).select(CargoDb::as_select()).load::<CargoDb>(conn) }).await
        }
        #[cfg(not(feature = "sqlite"))] { Err(prismar::RuntimeError::UnsupportedProvider("sqlite")) }
      }
      prismar::Provider::Postgres => {
        #[cfg(feature = "postgres")] {
      client.run_postgres(move |conn| { super::schema::cargoes::table.filter(super::schema::cargoes::namespace_name.eq(value)).select(CargoDb::as_select()).load::<CargoDb>(conn) }).await
        }
        #[cfg(not(feature = "postgres"))] { Err(prismar::RuntimeError::UnsupportedProvider("postgres")) }
      }
      prismar::Provider::MySql => {
        #[cfg(feature = "mysql")] {
      client.run_mysql(move |conn| { super::schema::cargoes::table.filter(super::schema::cargoes::namespace_name.eq(value)).select(CargoDb::as_select()).load::<CargoDb>(conn) }).await
        }
        #[cfg(not(feature = "mysql"))] { Err(prismar::RuntimeError::UnsupportedProvider("mysql")) }
      }
    };
    let mut rows = rows?;
    if let Some(filter) = filter {
      let mut filtered = Vec::new();
      for row in rows.drain(..) {
        if row.matches_filter(client, &filter).await? {
          filtered.push(row);
        }
      }
      return Ok(filtered);
    }
    Ok(rows)
  }

}

impl NamespaceDbFilter {
  pub fn cargos_some<T: prismar::TypedFilter>(mut self, filter: T) -> Self {
    self.inner = self.inner.relation("cargos", prismar::RelationFilterOp::Some, filter.into_model_filter());
    self
  }

  pub fn cargos_every<T: prismar::TypedFilter>(mut self, filter: T) -> Self {
    self.inner = self.inner.relation("cargos", prismar::RelationFilterOp::Every, filter.into_model_filter());
    self
  }

  pub fn cargos_none<T: prismar::TypedFilter>(mut self, filter: T) -> Self {
    self.inner = self.inner.relation("cargos", prismar::RelationFilterOp::None, filter.into_model_filter());
    self
  }

}

impl prismar::PrismaCreateData for NamespaceCreate {
  type Model = NamespaceDb;

  fn apply_defaults(&mut self) {
    let _ = self;
  }
}

impl prismar::PrismaUpdateData for NamespaceUpdate {
  type Model = NamespaceDb;
}

impl prismar::PrismaModel for NamespaceDb {
  type Create = NamespaceCreate;
  type Update = NamespaceUpdate;
  type Id = String;

  fn primary_key_field() -> &'static str { "name" }

  fn id(&self) -> Self::Id {
    self.name.clone()
  }

  fn id_from_filter(filter: prismar::ModelFilter) -> Result<Self::Id, prismar::RuntimeError> {
    let expected = "name";
    if filter.conditions.len() != 1 {
      return Err(prismar::RuntimeError::InvalidFilter(format!("expected a single equality predicate on {}", expected)));
    }

    match filter.conditions.into_iter().next().expect("one condition") {
      prismar::Condition::Predicate(predicate) if predicate.field == expected => {
                match predicate.filter {
          prismar::FieldFilter::String(prismar::StringFilter::Equals(value)) => Ok(value),
          _ => Err(prismar::RuntimeError::InvalidFilter("expected string equality on primary key".to_owned())),
        }
      }
      _ => Err(prismar::RuntimeError::InvalidFilter(format!("expected equality predicate on {}", expected))),
    }
  }

  fn id_from_create(data: &Self::Create) -> Option<Self::Id> {
    data.name.clone()
  }

  async fn matches_filter(&self, client: &prismar::PrismaClient, filter: &prismar::ModelFilter) -> Result<bool, prismar::RuntimeError> {
    for condition in &filter.conditions {
      let matched = match condition {
        prismar::Condition::Predicate(predicate) => match predicate.field.as_str() {
          "name" => prismar::evaluate_string_field(Some(self.name.as_str()), &predicate.filter),
          unknown => Err(prismar::RuntimeError::InvalidFilter(format!("unknown field '{}'", unknown))),
        }?,
        prismar::Condition::And(filters) => {
          let mut all_match = true;
          for inner in filters {
            if !std::boxed::Box::pin(self.matches_filter(client, inner)).await? {
              all_match = false;
              break;
            }
          }
          all_match
        }
        prismar::Condition::Or(filters) => {
          let mut any_match = false;
          for inner in filters {
            if std::boxed::Box::pin(self.matches_filter(client, inner)).await? {
              any_match = true;
              break;
            }
          }
          any_match
        }
        prismar::Condition::Not(inner) => !std::boxed::Box::pin(self.matches_filter(client, inner)).await?,
        prismar::Condition::Relation(relation) => match relation.field.as_str() {
          "cargos" => {
            let related = std::boxed::Box::pin(self.cargos(client, None)).await?;
            match relation.op {
              prismar::RelationFilterOp::Some => {
                let mut matched = false;
                for related in related {
                  if std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {
                    matched = true;
                    break;
                  }
                }
                Ok(matched)
              }
              prismar::RelationFilterOp::Every => {
                let mut matched = true;
                for related in related {
                  if !std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {
                    matched = false;
                    break;
                  }
                }
                Ok(matched)
              }
              prismar::RelationFilterOp::None => {
                let mut matched = true;
                for related in related {
                  if std::boxed::Box::pin(related.matches_filter(client, &relation.filter)).await? {
                    matched = false;
                    break;
                  }
                }
                Ok(matched)
              }
              prismar::RelationFilterOp::Is | prismar::RelationFilterOp::IsNot => Err(prismar::RuntimeError::InvalidFilter(format!("relation 'cargos' is to-many; use some/every/none"))),
            }
          },
          unknown => Err(prismar::RuntimeError::InvalidFilter(format!("unknown relation '{}'", unknown))),
        }?,
      };
      if !matched {
        return Ok(false);
      }
    }
    Ok(true)
  }

  async fn create(client: &prismar::PrismaClient, data: Self::Create) -> Result<usize, prismar::RuntimeError> {
match client.provider() {
      prismar::Provider::Sqlite => {
        #[cfg(feature = "sqlite")] {
      client.run_sqlite(move |conn| { diesel::insert_into(super::schema::namespaces::table).values(&data).execute(conn) }).await
        }
        #[cfg(not(feature = "sqlite"))] { Err(prismar::RuntimeError::UnsupportedProvider("sqlite")) }
      }
      prismar::Provider::Postgres => {
        #[cfg(feature = "postgres")] {
      client.run_postgres(move |conn| { diesel::insert_into(super::schema::namespaces::table).values(&data).execute(conn) }).await
        }
        #[cfg(not(feature = "postgres"))] { Err(prismar::RuntimeError::UnsupportedProvider("postgres")) }
      }
      prismar::Provider::MySql => {
        #[cfg(feature = "mysql")] {
      client.run_mysql(move |conn| { diesel::insert_into(super::schema::namespaces::table).values(&data).execute(conn) }).await
        }
        #[cfg(not(feature = "mysql"))] { Err(prismar::RuntimeError::UnsupportedProvider("mysql")) }
      }
    }  }

  async fn find_many(client: &prismar::PrismaClient, filter: Option<prismar::ModelFilter>) -> Result<Vec<Self>, prismar::RuntimeError> {
    let rows = match client.provider() {
      prismar::Provider::Sqlite => {
        #[cfg(feature = "sqlite")] {
      client.run_sqlite(|conn| { super::schema::namespaces::table.select(Self::as_select()).load::<Self>(conn) }).await
        }
        #[cfg(not(feature = "sqlite"))] { Err(prismar::RuntimeError::UnsupportedProvider("sqlite")) }
      }
      prismar::Provider::Postgres => {
        #[cfg(feature = "postgres")] {
      client.run_postgres(|conn| { super::schema::namespaces::table.select(Self::as_select()).load::<Self>(conn) }).await
        }
        #[cfg(not(feature = "postgres"))] { Err(prismar::RuntimeError::UnsupportedProvider("postgres")) }
      }
      prismar::Provider::MySql => {
        #[cfg(feature = "mysql")] {
      client.run_mysql(|conn| { super::schema::namespaces::table.select(Self::as_select()).load::<Self>(conn) }).await
        }
        #[cfg(not(feature = "mysql"))] { Err(prismar::RuntimeError::UnsupportedProvider("mysql")) }
      }
    };
    let mut rows = rows?;
    if let Some(filter) = filter {
      let mut filtered = Vec::new();
      for row in rows.drain(..) {
        if row.matches_filter(client, &filter).await? {
          filtered.push(row);
        }
      }
      return Ok(filtered);
    }
    Ok(rows)
  }

  async fn find_by_id(client: &prismar::PrismaClient, id: &Self::Id) -> Result<Option<Self>, prismar::RuntimeError> {
    let id = id.clone();
match client.provider() {
      prismar::Provider::Sqlite => {
        #[cfg(feature = "sqlite")] {
      client.run_sqlite(move |conn| { super::schema::namespaces::table.find(id).select(Self::as_select()).first::<Self>(conn).optional() }).await
        }
        #[cfg(not(feature = "sqlite"))] { Err(prismar::RuntimeError::UnsupportedProvider("sqlite")) }
      }
      prismar::Provider::Postgres => {
        #[cfg(feature = "postgres")] {
      client.run_postgres(move |conn| { super::schema::namespaces::table.find(id).select(Self::as_select()).first::<Self>(conn).optional() }).await
        }
        #[cfg(not(feature = "postgres"))] { Err(prismar::RuntimeError::UnsupportedProvider("postgres")) }
      }
      prismar::Provider::MySql => {
        #[cfg(feature = "mysql")] {
      client.run_mysql(move |conn| { super::schema::namespaces::table.find(id).select(Self::as_select()).first::<Self>(conn).optional() }).await
        }
        #[cfg(not(feature = "mysql"))] { Err(prismar::RuntimeError::UnsupportedProvider("mysql")) }
      }
    }  }

  async fn update_by_id(client: &prismar::PrismaClient, id: &Self::Id, data: Self::Update) -> Result<usize, prismar::RuntimeError> {
    let _ = (client, id, data);
    Ok(0)
  }

  async fn delete_by_id(client: &prismar::PrismaClient, id: &Self::Id) -> Result<usize, prismar::RuntimeError> {
    let id = id.clone();
match client.provider() {
      prismar::Provider::Sqlite => {
        #[cfg(feature = "sqlite")] {
      client.run_sqlite(move |conn| { diesel::delete(super::schema::namespaces::table.find(id)).execute(conn) }).await
        }
        #[cfg(not(feature = "sqlite"))] { Err(prismar::RuntimeError::UnsupportedProvider("sqlite")) }
      }
      prismar::Provider::Postgres => {
        #[cfg(feature = "postgres")] {
      client.run_postgres(move |conn| { diesel::delete(super::schema::namespaces::table.find(id)).execute(conn) }).await
        }
        #[cfg(not(feature = "postgres"))] { Err(prismar::RuntimeError::UnsupportedProvider("postgres")) }
      }
      prismar::Provider::MySql => {
        #[cfg(feature = "mysql")] {
      client.run_mysql(move |conn| { diesel::delete(super::schema::namespaces::table.find(id)).execute(conn) }).await
        }
        #[cfg(not(feature = "mysql"))] { Err(prismar::RuntimeError::UnsupportedProvider("mysql")) }
      }
    }  }
}
