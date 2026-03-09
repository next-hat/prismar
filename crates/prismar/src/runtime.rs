use diesel::{
  Connection,
  r2d2::{ConnectionManager, Pool},
};
use thiserror::Error;

#[derive(Debug)]
pub struct Prismar<Conn>
where
  Conn: Connection + diesel::r2d2::R2D2Connection + 'static,
{
  pool: Pool<ConnectionManager<Conn>>,
  backend: crate::SqlBackend,
}

impl<Conn> Clone for Prismar<Conn>
where
  Conn: Connection + diesel::r2d2::R2D2Connection + 'static,
{
  fn clone(&self) -> Self {
    Self {
      pool: self.pool.clone(),
      backend: self.backend,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
  Sqlite,
  Postgres,
  MySql,
}

#[derive(Clone)]
pub struct PrismaClient {
  inner: PrismaClientInner,
}

#[derive(Clone)]
enum PrismaClientInner {
  #[cfg(feature = "sqlite")]
  Sqlite(Prismar<diesel::sqlite::SqliteConnection>),
  #[cfg(feature = "postgres")]
  Postgres(Prismar<diesel::pg::PgConnection>),
  #[cfg(feature = "mysql")]
  MySql(Prismar<diesel::mysql::MysqlConnection>),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
  #[error("failed to build diesel connection pool: {0}")]
  PoolBuild(String),
  #[error("provider support is not compiled in: {0}")]
  UnsupportedProvider(&'static str),
  #[error("provider mismatch: expected {expected}, actual {actual}")]
  ProviderMismatch {
    expected: &'static str,
    actual: &'static str,
  },
  #[error("invalid filter: {0}")]
  InvalidFilter(String),
  #[error("failed to acquire diesel connection from pool: {0}")]
  Pool(String),
  #[error("blocking query task interrupted: {0}")]
  Join(String),
  #[error("diesel query failed: {0}")]
  Query(String),
}

impl PrismaClient {
  pub fn new(
    provider: Provider,
    database_url: impl Into<String>,
  ) -> Result<Self, RuntimeError> {
    let database_url = database_url.into();
    let inner = match provider {
      Provider::Sqlite => {
        #[cfg(feature = "sqlite")]
        {
          PrismaClientInner::Sqlite(Prismar::new(
            database_url,
            crate::SqlBackend::Sqlite,
          )?)
        }
        #[cfg(not(feature = "sqlite"))]
        {
          return Err(RuntimeError::UnsupportedProvider("sqlite"));
        }
      }
      Provider::Postgres => {
        #[cfg(feature = "postgres")]
        {
          PrismaClientInner::Postgres(Prismar::new(
            database_url,
            crate::SqlBackend::Postgres,
          )?)
        }
        #[cfg(not(feature = "postgres"))]
        {
          return Err(RuntimeError::UnsupportedProvider("postgres"));
        }
      }
      Provider::MySql => {
        #[cfg(feature = "mysql")]
        {
          PrismaClientInner::MySql(Prismar::new(
            database_url,
            crate::SqlBackend::MySql,
          )?)
        }
        #[cfg(not(feature = "mysql"))]
        {
          return Err(RuntimeError::UnsupportedProvider("mysql"));
        }
      }
    };
    Ok(Self { inner })
  }

  pub fn provider(&self) -> Provider {
    match &self.inner {
      #[cfg(feature = "sqlite")]
      PrismaClientInner::Sqlite(_) => Provider::Sqlite,
      #[cfg(feature = "postgres")]
      PrismaClientInner::Postgres(_) => Provider::Postgres,
      #[cfg(feature = "mysql")]
      PrismaClientInner::MySql(_) => Provider::MySql,
    }
  }

  pub async fn create<D>(&self, data: D) -> Result<usize, RuntimeError>
  where
    D: crate::PrismaCreateData,
  {
    let mut data = data;
    data.apply_defaults();
    <D::Model as crate::PrismaModel>::create(self, data).await
  }

  pub async fn find_many<M>(
    &self,
    filter: Option<crate::ModelFilter>,
  ) -> Result<Vec<M>, RuntimeError>
  where
    M: crate::PrismaModel,
  {
    M::find_many(self, filter).await
  }

  pub async fn find_all<M>(&self) -> Result<Vec<M>, RuntimeError>
  where
    M: crate::PrismaModel,
  {
    self.find_many::<M>(None).await
  }

  pub async fn find_by_id<M>(
    &self,
    id: &M::Id,
  ) -> Result<Option<M>, RuntimeError>
  where
    M: crate::PrismaModel,
  {
    M::find_by_id(self, id).await
  }

  pub async fn update_by_id<D>(
    &self,
    id: &<D::Model as crate::PrismaModel>::Id,
    data: D,
  ) -> Result<usize, RuntimeError>
  where
    D: crate::PrismaUpdateData,
  {
    <D::Model as crate::PrismaModel>::update_by_id(self, id, data).await
  }

  pub async fn delete_by_id<M>(&self, id: &M::Id) -> Result<usize, RuntimeError>
  where
    M: crate::PrismaModel,
  {
    M::delete_by_id(self, id).await
  }

  pub async fn update<M, F>(
    &self,
    filter: F,
    data: M::Update,
  ) -> Result<usize, RuntimeError>
  where
    M: crate::PrismaModel,
    F: crate::TypedFilter,
  {
    let id = M::id_from_filter(filter.into_model_filter())?;
    M::update_by_id(self, &id, data).await
  }

  pub async fn delete<M, F>(&self, filter: F) -> Result<usize, RuntimeError>
  where
    M: crate::PrismaModel,
    F: crate::TypedFilter,
  {
    let id = M::id_from_filter(filter.into_model_filter())?;
    M::delete_by_id(self, &id).await
  }

  #[cfg(feature = "sqlite")]
  pub async fn run_sqlite<R, F>(&self, operation: F) -> Result<R, RuntimeError>
  where
    R: Send + 'static,
    F: FnOnce(&mut diesel::sqlite::SqliteConnection) -> diesel::QueryResult<R>
      + Send
      + 'static,
  {
    match &self.inner {
      PrismaClientInner::Sqlite(client) => client.run(operation).await,
      _ => Err(RuntimeError::ProviderMismatch {
        expected: "sqlite",
        actual: self.provider_name(),
      }),
    }
  }

  #[cfg(feature = "postgres")]
  pub async fn run_postgres<R, F>(
    &self,
    operation: F,
  ) -> Result<R, RuntimeError>
  where
    R: Send + 'static,
    F: FnOnce(&mut diesel::pg::PgConnection) -> diesel::QueryResult<R>
      + Send
      + 'static,
  {
    match &self.inner {
      PrismaClientInner::Postgres(client) => client.run(operation).await,
      _ => Err(RuntimeError::ProviderMismatch {
        expected: "postgres",
        actual: self.provider_name(),
      }),
    }
  }

  #[cfg(feature = "mysql")]
  pub async fn run_mysql<R, F>(&self, operation: F) -> Result<R, RuntimeError>
  where
    R: Send + 'static,
    F: FnOnce(&mut diesel::mysql::MysqlConnection) -> diesel::QueryResult<R>
      + Send
      + 'static,
  {
    match &self.inner {
      PrismaClientInner::MySql(client) => client.run(operation).await,
      _ => Err(RuntimeError::ProviderMismatch {
        expected: "mysql",
        actual: self.provider_name(),
      }),
    }
  }

  fn provider_name(&self) -> &'static str {
    match self.provider() {
      Provider::Sqlite => "sqlite",
      Provider::Postgres => "postgres",
      Provider::MySql => "mysql",
    }
  }
}

impl<Conn> Prismar<Conn>
where
  Conn: Connection + diesel::r2d2::R2D2Connection + 'static,
{
  pub fn new(
    database_url: impl Into<String>,
    backend: crate::SqlBackend,
  ) -> Result<Self, RuntimeError> {
    Ok(Self {
      pool: connection_pool(database_url)?,
      backend,
    })
  }

  pub fn from_pool(
    pool: Pool<ConnectionManager<Conn>>,
    backend: crate::SqlBackend,
  ) -> Self {
    Self { pool, backend }
  }

  pub fn backend(&self) -> crate::SqlBackend {
    self.backend
  }

  pub fn pool(&self) -> Pool<ConnectionManager<Conn>> {
    self.pool.clone()
  }

  pub async fn run<R, F>(&self, operation: F) -> Result<R, RuntimeError>
  where
    R: Send + 'static,
    F: FnOnce(&mut Conn) -> diesel::QueryResult<R> + Send + 'static,
  {
    with_connection(self.pool.clone(), operation).await
  }

  pub async fn run_raw<R, F>(
    &self,
    query: crate::RawSqlQuery,
    operation: F,
  ) -> Result<R, RuntimeError>
  where
    R: Send + 'static,
    F: FnOnce(&mut Conn, crate::RawSqlQuery) -> diesel::QueryResult<R>
      + Send
      + 'static,
  {
    with_raw_query(self.pool.clone(), query, operation).await
  }
}

pub fn connection_pool<Conn>(
  database_url: impl Into<String>,
) -> Result<Pool<ConnectionManager<Conn>>, RuntimeError>
where
  Conn: Connection + diesel::r2d2::R2D2Connection + 'static,
{
  let manager = ConnectionManager::<Conn>::new(database_url.into());
  Pool::builder()
    .build(manager)
    .map_err(|err| RuntimeError::PoolBuild(err.to_string()))
}

pub async fn with_connection<Conn, R, F>(
  pool: Pool<ConnectionManager<Conn>>,
  operation: F,
) -> Result<R, RuntimeError>
where
  Conn: Connection + diesel::r2d2::R2D2Connection + 'static,
  R: Send + 'static,
  F: FnOnce(&mut Conn) -> diesel::QueryResult<R> + Send + 'static,
{
  ntex::rt::spawn_blocking(move || {
    let mut connection = pool
      .get()
      .map_err(|err| RuntimeError::Pool(err.to_string()))?;
    operation(&mut connection)
      .map_err(|err| RuntimeError::Query(err.to_string()))
  })
  .await
  .map_err(|err| RuntimeError::Join(err.to_string()))?
}

pub async fn with_raw_query<Conn, R, F>(
  pool: Pool<ConnectionManager<Conn>>,
  query: crate::RawSqlQuery,
  operation: F,
) -> Result<R, RuntimeError>
where
  Conn: Connection + diesel::r2d2::R2D2Connection + 'static,
  R: Send + 'static,
  F: FnOnce(&mut Conn, crate::RawSqlQuery) -> diesel::QueryResult<R>
    + Send
    + 'static,
{
  with_connection(pool, move |conn| operation(conn, query)).await
}
