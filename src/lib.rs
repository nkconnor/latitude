//! Latitude is a library for dynamic runtime DDL based on [`sqlx`](https://github.com/launchbadge/sqlx)
//! and [`barrel`](https://github.com/rust-db/barrel). **_NOTE_**: This project
//! is in early development along with `sqlx`. Please use at your own risk and strongly
//! consider using a better tested and polished project such as
//! [Refinery](https://github.com/rust-db/refinery). There will definitely be API changes
//! in the near future.
//!
//! Originally this was intended to to be a migration toolkit, but, it has been slimmed down. It's
//! unclear if we will still pursue a migration oriented API. Adding migration capability would be
//! a feature addition as opposed to a rewrite.
//! With the migratio changes this library could be considered amore portable but less
//! accessible alternative to `sqlx-cli`: users require less concern over understanding and
//! maintaining multiple SQL dialects, but, must have familiarity with Rust to get up and
//! running. It may be a good fit if your application is already written in Rust and either:
//!     
//! - you want migration compatibility across multiple databases (e.g. if you are using MySQL
//!     in production and SQLite for development); or
//!
//! - you just like writing in the DSL over plain SQL
//!
//! Please raise an issue on GitHub if you have suggestions, feedback, bug reports, or
//! otherwise.
//!
//! ## Getting Started
//!
//! ```toml
//! latitude = 0.0.1
//! ```
//!
//! ## Usage
//!
//! ```
//! use latitude::prelude::*;
//!
//! let connection = Connection::new("sqlite::memory:").await?;
//!
//! table::create("users")
//!       .column("name", varchar(255))
//!       .column("age",  integer())
//!       .column("xyx",  boolean())
//!       .execute(&mut connection)
//!       .await?
//! ```
use async_trait::async_trait;
use barrel::Migration;
use sqlx::{any::AnyConnection, any::AnyDone, any::AnyKind, Any};
use std::str::FromStr;
use thiserror::Error;

pub use barrel::types;

/// Exports commonly used types and modules
pub mod prelude {
    pub use crate::table;
    pub use crate::types;
    pub use crate::types::boolean;
    pub use crate::types::date;
    pub use crate::types::integer;
    pub use crate::types::primary;
    pub use crate::types::varchar;
    pub use crate::Connection;
    pub use crate::Statement;
}

/// DDL operations for tables such as `CREATE`, `DROP`, and `ALTER`
///
/// # Examples
///
/// ```
/// use latitude::{table, types};
///
/// table::create("users")
///       .column("id", types::primary())
///       .column("name", types::varchar(50))
///       .if_not_exists();
/// ```
pub mod table {

    use barrel::types::Type;

    /// A `CREATE TABLE` statement
    pub struct CreateTable {
        pub(crate) name: String,
        pub(crate) columns: Vec<(String, Type)>,
        pub(crate) if_not_exists: bool,
    }

    impl CreateTable {
        /// Add the `IF NOT EXISTS` qualifier to the create table
        /// statement
        pub fn if_not_exists(mut self) -> Self {
            self.if_not_exists = true;
            self
        }

        /// Add a column to the create table statement
        pub fn column<N: Into<String>>(mut self, name: N, _type: Type) -> Self {
            self.columns.push((name.into(), _type));
            self
        }
    }

    /// Create a `CREATE TABLE` statement for the provided table `name`
    ///
    /// # Examples
    ///
    /// ```
    /// use latitude::{table, types};
    ///
    /// table::create("users")
    ///       .column("id", types::primary())
    ///       .column("name", types::varchar(50))
    ///       .if_not_exists();
    /// ```
    pub fn create<N: Into<String>>(name: N) -> CreateTable {
        CreateTable {
            name: name.into(),
            columns: Vec::new(),
            if_not_exists: false,
        }
    }

    /// A `DROP TABLE` statement
    pub struct DropTable {
        pub(crate) name: String,
        pub(crate) if_exists: bool,
    }

    impl DropTable {
        /// Add the `IF EXISTS` qualifier to this drop table statement
        pub fn if_exists(mut self) -> Self {
            self.if_exists = true;
            self
        }
    }

    /// Create a `DROP TABLE` statement for the provided table `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// use latitude::table;
    ///
    /// table::drop("users")
    ///       .if_exists();
    /// ```
    pub fn drop<N: Into<String>>(name: N) -> DropTable {
        DropTable {
            name: name.into(),
            if_exists: false,
        }
    }
}

/// A database connection wrapper over `sqlx::Connection`. I think that this is necessary
/// for now, but, it should be removed in the not-so-far future.
pub struct Connection {
    uri: String,
    inner: AnyConnection,
}

impl Connection {
    /// Create a new `Connection` from the given `uri`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::Connection;
    /// Connection::new("sqlite::memory:").unwrap();
    /// ```
    pub async fn new<URI: Into<String>>(uri: URI) -> Result<Self, sqlx::Error> {
        use sqlx::Connection;
        let uri = uri.into();
        let inner = AnyConnection::connect(uri.clone().as_str()).await?;
        Ok(Self { uri, inner })
    }

    /// Parse the SQL dialect for this connection's URI.
    ///
    /// Note: `sqlx::AnyKind` does not derive `Clone`, so this convenience function
    /// gives us `AnyKind` from self reference on demand
    fn dialect(&self) -> Result<AnyKind, sqlx::Error> {
        AnyKind::from_str(self.uri.as_str())
    }
}

#[derive(Debug, Error)]
enum Error {
    #[error("Attempted to use an unsupported SQL dialect; available options are SQLite and MySQL")]
    UnsupportedDialect,
}

/// An executable DDL Statement.
#[async_trait]
pub trait Statement: Sized {
    /// Create a String DDL statement for the given SQL Dialect
    fn for_dialect(self, dialect: AnyKind) -> Result<String, sqlx::Error>;

    /// Execute the DDL statement
    async fn execute(self, conn: &mut Connection) -> Result<AnyDone, sqlx::Error> {
        let dialect = conn.dialect()?;
        let statement = self.for_dialect(dialect)?;
        sqlx::query::<Any>(statement.as_str())
            .execute(&mut conn.inner)
            .await
    }
}

impl Statement for table::CreateTable {
    fn for_dialect(self, dialect: AnyKind) -> Result<String, sqlx::Error> {
        let mut migration = Migration::new();
        if self.if_not_exists {
            migration.create_table_if_not_exists(self.name.clone(), move |table| {
                for (name, ty) in self.columns.clone() {
                    table.add_column(name, ty);
                }
            });
        }

        migration.for_dialect(dialect)
    }
}

impl Statement for Migration {
    fn for_dialect(self, dialect: AnyKind) -> Result<String, sqlx::Error> {
        let variant = match dialect {
            AnyKind::MySql => Some(barrel::backend::SqlVariant::Mysql),
            AnyKind::Sqlite => Some(barrel::backend::SqlVariant::Sqlite),
            _ => None,
        };

        let variant = variant
            .map(Ok)
            .unwrap_or(Err(sqlx::Error::Configuration(Box::new(
                Error::UnsupportedDialect,
            ))))?;

        Ok(self.make_from(variant))
    }
}

//impl Latitude {
//    pub fn new<I: IntoIterator<Item = Transition>>(transitions: I) -> Self {
//        Self {
//            transitions: transitions.into_iter().collect(),
//        }
//    }
//
//    async fn _migrate(
//        self,
//        mut conn: AnyConnection,
//        variant: barrel::backend::SqlVariant,
//    ) -> Result<(), sqlx::Error> {
//        let mut global = conn.begin().await?;
//
//        let mut transaction = global.begin().await?;
//
//        // Create internal table if not exists
//        let mut migration = Migration::new();
//        migration.drop_table(name)
//        migration.create_table_if_not_exists("_latitude_transitions", |table| {
//            table.add_column("id", types::primary());
//        });
//
//        let _done = sqlx::query::<Any>(migration.make_from(variant).as_str())
//            .execute(&mut transaction)
//            .await?;
//
//        // Apply each transition from max_id+1 onward
//        let max_id: Option<i64> = sqlx::query::<Any>("SELECT MAX(id) FROM _latitude_transitions")
//            .map(|row: AnyRow| row.try_get(0).unwrap())
//            .fetch_optional(&mut transaction)
//            .await
//            .unwrap();
//
//        //let ids: Vec<i64> = sqlx::query::<Any>("SELECT id FROM _latitude_transitions")
//        //    .map(|row: AnyRow| row.try_get(0).unwrap())
//        //    .fetch_all(&mut transaction)
//        //    .await
//        //    .unwrap();
//
//        // assert: SQL IDs start at 1
//        //println!("All IDs are {:?}", ids);
//        println!("Max ID was: {:?}", max_id);
//
//        let skip_from = match max_id {
//            // TODO, SQLITE returns MAX(id) = 0 when there are no rows;
//            // need to see if this is consistent (unlikely), and special case or
//            // refactor appropriately
//            Some(max) if max > 0 => max as usize,
//            _ => 0,
//        };
//
//        transaction.commit().await?;
//
//        for (id, transition) in self
//            .transitions
//            .into_iter()
//            .enumerate()
//            .map(|(i, v)| (i + 1, v))
//            .skip(skip_from)
//        {
//            let mut transaction = global.begin().await?;
//
//            // Apply the forward migration
//            sqlx::query::<Any>(transition.up.make_from(variant).as_str())
//                .execute(&mut transaction)
//                .await?;
//
//            // Preserve migration state
//            sqlx::query::<Any>("INSERT INTO _latitude_transitions(id) VALUES(?)")
//                .bind(id as i64)
//                .execute(&mut transaction)
//                .await?;
//
//            transaction.commit().await?;
//        }
//
//        //let table_count: i64 = sqlx::query::<Any>("SELECT COUNT(1) FROM _latitude_transitions")
//        //    .map(|row: AnyRow| row.try_get(0).unwrap())
//        //    .fetch_one(&mut global)
//        //    .await?;
//
//        global.commit().await?;
//
//        Ok(())
//    }
//
//    /// Migrate the latest transitions. This compares existing
//    /// database state to the runtime-defined migrations you supplied in
//    /// `Latitude::new`.
//    pub async fn migrate(mut self, uri: &str) -> Result<(), sqlx::Error> {
//        let variant = match AnyKind::from_str(uri).unwrap() {
//            AnyKind::MySql => barrel::backend::SqlVariant::Mysql,
//            AnyKind::Sqlite => barrel::backend::SqlVariant::Sqlite,
//            _ => panic!(),
//        };
//
//        let mut conn = AnyConnection::connect(uri).await?;
//        self._migrate(conn, variant).await
//    }
//}

#[cfg(test)]
mod tests {

    #[tokio::test(threaded_scheduler)]
    async fn test_example_todos() {
        use crate::{table, Statement};
        use sqlx::AnyConnection;
    }
}
