use prismar::diesel::{OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};

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
    }
  }

  async fn find_many(client: &prismar::PrismaClient, filter: Option<prismar::ModelFilter>) -> Result<Vec<Self>, prismar::RuntimeError> {
    if let Some(filter) = filter {
      let id = Self::id_from_filter(filter)?;
      return Ok(Self::find_by_id(client, &id).await?.into_iter().collect());
    }
    match client.provider() {
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
    }
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
    }
  }

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
    }
  }
}
