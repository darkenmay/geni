#[cfg(test)]
mod tests {
    use crate::config::Database;
    use crate::database_drivers;
    
    use log::info;
    use serial_test::serial;
    

    use crate::migrate::{down, up};
    use anyhow::Ok;
    use anyhow::Result;
    use chrono::Utc;
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use std::{fs::File, vec};
    use tokio::test;

    fn generate_test_migrations(migration_path: String) -> Result<()> {
        let file_endings = vec!["up", "down"];
        let test_queries = [
            (
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL); CREATE TABLE computers (id INTEGER PRIMARY KEY, name TEXT NOT NULL); ;",
                "DROP TABLE users;",
            ),
            (
                "CREATE TABLE users2 (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "DROP TABLE users2;",
            ),
            (
                "CREATE TABLE users3 (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "DROP TABLE users3;",
            ),
            (
                "CREATE TABLE users4 (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "DROP TABLE users4;",
            ),
            (
                "CREATE TABLE users5 (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "DROP TABLE users5;",
            ),
            (
                "CREATE TABLE users6 (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "DROP TABLE users6;",
            ),
        ];

        for (index, t) in test_queries.iter().enumerate() {
            for f in &file_endings {
                let timestamp = Utc::now().timestamp() + index as i64;

                let filename = format!("{migration_path}/{timestamp}_{index}_test.{f}.sql");
                let filename_str = filename.as_str();
                let path = std::path::Path::new(filename_str);

                // Generate the folder if it don't exist
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut file = File::create(path)?;

                match *f {
                    "up" => {
                        file.write_all(t.0.as_bytes())?;
                    }
                    "down" => {
                        file.write_all(t.1.as_bytes())?;
                    }
                    _ => {}
                }

                info!("Generated {}", filename_str)
            }
        }

        Ok(())
    }

    async fn test_migrate(database: Database, db_url: &str) -> Result<()> {
        let url = db_url.to_string();
        let tmp_dir =
            tempdir::TempDir::new(format!("test_migrate_{}", database.as_str().unwrap()).as_str())
                .unwrap();
        let migration_folder = tmp_dir.path();
        let migration_folder_string = migration_folder.to_str().unwrap().to_string();

        let database_wait_timeout = 30;

        let database_schema_file =
            env::var("DATABASE_SCHEMA_FILE").unwrap_or("schema.sql".to_string());

        generate_test_migrations(migration_folder_string.to_string()).unwrap();

        let mut create_client = database_drivers::new(
            url.clone(),
            None,
            "schema_migrations".to_string(),
            migration_folder_string.clone(),
            database_schema_file.clone(),
            Some(database_wait_timeout),
            false,
        )
        .await
        .unwrap();
        match database {
            // we don't need to match sqlite as sqlite driver creates the file if it doesn't exist
            Database::Postgres | Database::MySQL | Database::MariaDB => {
                create_client.create_database().await.unwrap();
            }
            _ => {}
        };

        let mut client = database_drivers::new(
            url.clone(),
            None,
            "schema_migrations".to_string(),
            migration_folder_string.clone(),
            database_schema_file.clone(),
            Some(database_wait_timeout),
            true,
        )
        .await
        .unwrap();

        let u = up(
            url.clone(),
            None,
            "schema_migrations".to_string(),
            migration_folder_string.clone(),
            database_schema_file.clone(),
            Some(database_wait_timeout),
            true,
        )
        .await;
        assert!(u.is_ok());

        assert_eq!(
            client
                .get_or_create_schema_migrations()
                .await
                .unwrap()
                .len(),
            6,
        );

        let d = down(
            url.clone(),
            None,
            "schema_migrations".to_string(),
            migration_folder_string.clone(),
            database_schema_file.clone(),
            Some(database_wait_timeout),
            false,
            &1,
        )
        .await;
        assert!(d.is_ok());

        assert_eq!(
            client
                .get_or_create_schema_migrations()
                .await
                .unwrap()
                .len(),
            5
        );

        let d = down(
            url,
            None,
            "schema_migrations".to_string(),
            migration_folder_string.clone(),
            database_schema_file.clone(),
            Some(database_wait_timeout),
            false,
            &3,
        )
        .await;
        assert!(d.is_ok());

        assert_eq!(
            client
                .get_or_create_schema_migrations()
                .await
                .unwrap()
                .len(),
            2
        );

        let schema_dump_file = format!("{}/{}", migration_folder_string, database_schema_file);
        let file = Path::new(&schema_dump_file);
        assert!(file.exists());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_migrate_libsql() -> Result<()> {
        env::set_var("DATABASE_SCHEMA_FILE", "libsql_schema.sql");
        let url = "http://localhost:6000";
        test_migrate(Database::LibSQL, url).await
    }

    #[test]
    #[serial]
    async fn test_migrate_postgres() -> Result<()> {
        env::set_var("DATABASE_SCHEMA_FILE", "postgres_schema.sql");
        let url = "psql://postgres:mysecretpassword@localhost:6437/app?sslmode=disable";
        test_migrate(Database::Postgres, url).await
    }

    #[test]
    #[serial]
    async fn test_migrate_mysql() -> Result<()> {
        env::set_var("DATABASE_SCHEMA_FILE", "mysql_schema.sql");
        let url = "mysql://root:password@localhost:3306/app";
        test_migrate(Database::MySQL, url).await
    }

    #[test]
    #[serial]
    async fn test_migrate_maria() -> Result<()> {
        env::set_var("DATABASE_SCHEMA_FILE", "maria_schema.sql");
        let url = "mariadb://root:password@localhost:3307/app";
        test_migrate(Database::MariaDB, url).await
    }

    #[test]
    #[serial]
    async fn test_migrate_sqlite() -> Result<()> {
        env::set_var("DATABASE_SCHEMA_FILE", "sqlite_schema.sql");
        let tmp_dir = tempdir::TempDir::new("temp_migrate_sqlite_db").unwrap();
        let migration_folder = tmp_dir.path();
        let migration_folder_string = migration_folder.to_str().unwrap();
        let filename = format!("{migration_folder_string}/test.sqlite");
        let filename_str = filename.as_str();
        let path = std::path::Path::new(filename_str);

        File::create(path)?;

        let url = format!("sqlite://{}", path.to_str().unwrap());

        test_migrate(Database::SQLite, &url).await
    }

    #[test]
    #[serial]
    async fn test_migrate_failure() -> Result<()> {
        env::set_var("DATABASE_SCHEMA_FILE", "sqlite_schema.sql");
        let tmp_dir = tempdir::TempDir::new("temp_migrate_sqlite_db").unwrap();
        let migration_folder = tmp_dir.path();
        let migration_folder_string = migration_folder.to_str().unwrap();
        let filename = format!("{migration_folder_string}/test.sqlite");
        let filename_str = filename.as_str();
        let path = std::path::Path::new(filename_str);

        File::create(path).unwrap();

        let url = format!("sqlite://{}", path.to_str().unwrap());
        let tmp_dir = tempdir::TempDir::new(
            format!("test_migrate_{}", Database::SQLite.as_str().unwrap()).as_str(),
        )
        .unwrap();
        let migration_folder = tmp_dir.path();
        let migration_folder_string = migration_folder.to_str().unwrap().to_string();

        let database_wait_timeout = 30;

        let database_schema_file =
            env::var("DATABASE_SCHEMA_FILE").unwrap_or("schema.sql".to_string());

        let file_endings = vec!["up", "down"];
        let test_queries = [(
            r#"
                    CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

                    CREATE OR REPLACE FUNCTION uuid_generate_v7()
                    RETURN uuid
                    AS $$
                    SELECT encode(
                        set_bit(
                        set_bit(
                            overlay(uuid_send(gen_random_uuid())
                                    placing substring(int8send(floor(extract(epoch from clock_timestamp()) * 1000)::bigint) from 3)
                                    from 1 for 6
                            ),
                            52, 1
                        ),
                        53, 1
                        ),
                        'hex')::uuid;
                    $$
                    LANGUAGE SQL
                    VOLATILE;

                    CREATE TABLE tokens (
                        id UUID NOT NULL DEFAULT uuid_generate_v7 ()
                        token TEXT NOT NULL UNIQUE,
                        app_id UUID NOT NULL,
                        app_slug TEXT NOT NULL,
                        expires_at TIMESTAMP,
                        updated_at TIMESTAMP WITH TIME ZONE,
                        created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                    );

                    CREATE TABLE permissions (
                        id UUID NOT NULL DEFAULT uuid_generate_v7 ()
                        token UUID NOT NULL,
                        updated_at TIMESTAMP WITH TIME ZONE,
                        created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                    );
                "#,
            "",
        )];

        for (index, t) in test_queries.iter().enumerate() {
            for f in &file_endings {
                let timestamp = Utc::now().timestamp() + index as i64;

                let filename =
                    format!("{migration_folder_string}/{timestamp}_{index}_test.{f}.sql");
                let filename_str = filename.as_str();
                let path = std::path::Path::new(filename_str);

                // Generate the folder if it don't exist
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).unwrap()
                }

                let mut file = File::create(path).unwrap();

                match *f {
                    "up" => file.write_all(t.0.as_bytes()).unwrap(),
                    "down" => file.write_all(t.1.as_bytes()).unwrap(),
                    _ => {}
                }

                info!("Generated {}", filename_str)
            }
        }

        let u = up(
            url.clone(),
            None,
            "schema_migrations".to_string(),
            migration_folder_string.clone(),
            database_schema_file.clone(),
            Some(database_wait_timeout),
            true,
        )
        .await;
        assert!(u.is_err());

        Ok(())
    }
}
