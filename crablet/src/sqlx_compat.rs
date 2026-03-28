pub use sqlx_core::acquire::Acquire;
pub use sqlx_core::error::Error;
pub use sqlx_core::from_row::FromRow;
pub use sqlx_core::query::query;
pub use sqlx_core::query_as::query_as;
pub use sqlx_core::query_scalar::query_scalar;
pub use sqlx_core::row::Row;

pub mod migrate {
  pub use sqlx_core::migrate::*;
}

pub mod sqlite {
  pub use sqlx_sqlite::{
    Sqlite,
    SqliteConnection,
    SqliteExecutor,
    SqlitePool,
    SqlitePoolOptions,
    SqliteQueryResult,
    SqliteRow,
    SqliteStatement,
    SqliteTransaction,
  };
}

pub use sqlx_sqlite::{Sqlite, SqlitePool};
