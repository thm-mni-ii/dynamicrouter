use crate::config::CONFIG;
use crate::fatal;
use once_cell::sync::OnceCell;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use std::time::Duration;

pub static DB: OnceCell<Db> = OnceCell::new();

pub async fn init_db() {
    if DB.get().is_some() {
        return;
    }
    match Db::connect().await {
        Ok(db) => {
            if let Err(err) = sqlx::migrate!().run(&db.executor).await {
                eprintln!("Error while running migrations: {err}");
                fatal!("Error while running migrations: {err}");
            }
            let _ = DB.set(db);
            let _ = crate::db::request::cleanup_stale_queued_requests().await;
            if let Err(err) = crate::db::legacy_migration::check_and_migrate_legacy_config().await {
                eprintln!("Error during legacy config migration: {err}");
                fatal!("Error during legacy config migration: {err}");
            }
        }
        Err(err) => {
            eprintln!("Error while connecting to database: {err}");
            fatal!("Error while connection to database: {err}");
        }
    }
}

pub fn get_executor() -> &'static Pool<Sqlite> {
    DB.get()
        .expect("you tried to access the db before it was initialized")
        .get_executor()
}

pub struct Db {
    executor: Pool<Sqlite>,
}

impl Db {
    pub async fn connect() -> Result<Db, anyhow::Error> {
        let db_url = &CONFIG.database_url;
        let connect_opts = if db_url.starts_with("sqlite:") {
            SqliteConnectOptions::from_str(db_url)?
        } else {
            SqliteConnectOptions::from_str(&format!("sqlite://{db_url}"))?
        }
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(10));

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(connect_opts)
            .await?;

        Ok(Db { executor: pool })
    }

    pub fn get_executor(&self) -> &Pool<Sqlite> {
        &self.executor
    }
}
