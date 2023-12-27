use crate::config;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub mod libsql;
pub mod maria;
pub mod mysql;
pub mod postgres;
pub mod sqlite;
mod utils;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SchemaMigration {
    pub id: String,
}

// DatabaseDriver is a trait that all database drivers must implement
pub trait DatabaseDriver {
    // execute giving query for the specifiv
    fn execute<'a>(
        &'a mut self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>>;

    // create database with the specific driver
    fn create_database(&mut self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>>;

    // drop database with the specific driver
    fn drop_database(&mut self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>>;

    // get current schema migrations for the schema migrations table
    fn get_or_create_schema_migrations(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, anyhow::Error>> + '_>>;

    // insert new schema migration
    fn insert_schema_migration<'a>(
        &'a mut self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>>;

    // remove schema migration from the schema migrations table
    fn remove_schema_migration<'a>(
        &'a mut self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>>;

    // create database with the specific driver
    fn ready(&mut self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>>;
}

// Creates a new database driver based on the database_url
pub async fn new(
    db_url: &str,
    with_selected_database: bool,
) -> Result<Box<dyn DatabaseDriver>, anyhow::Error> {
    let mut parsed_db_url = url::Url::parse(db_url)?;

    let cloned_db_url = parsed_db_url.clone();
    let mut database_name = cloned_db_url.path();

    let driver = config::Database::new(parsed_db_url.scheme())?;

    if let (
        false,
        config::Database::MariaDB | config::Database::Postgres | config::Database::MySQL,
    ) = (with_selected_database, driver)
    {
        parsed_db_url.set_path("");
        database_name = cloned_db_url.path().trim_start_matches('/');
    }

    let scheme = parsed_db_url.scheme();

    match scheme {
        "http" | "https" => {
            let token = config::database_token();
            let driver = libsql::LibSQLDriver::new(parsed_db_url.as_str(), token).await?;
            Ok(Box::new(driver))
        }
        "postgres" | "psql" => {
            let driver =
                postgres::PostgresDriver::new(parsed_db_url.as_str(), database_name).await?;
            Ok(Box::new(driver))
        }
        "mysql" => {
            let driver = mysql::MySQLDriver::new(parsed_db_url.as_str(), database_name).await?;
            Ok(Box::new(driver))
        }
        "sqlite" | "sqlite3" => {
            let driver = sqlite::SqliteDriver::new(db_url).await?;
            Ok(Box::new(driver))
        }
        "mariadb" => {
            let driver = maria::MariaDBDriver::new(parsed_db_url.as_str(), database_name).await?;
            Ok(Box::new(driver))
        }
        _ => bail!("Unsupported database driver: {}", scheme),
    }
}
