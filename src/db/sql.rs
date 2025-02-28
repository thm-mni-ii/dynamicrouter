use std::process::exit;
use log::error;
use once_cell::sync::OnceCell;
use crate::config::CONFIG;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

pub static DB: OnceCell<Db> = OnceCell::new();

pub async fn init_db() {
    match Db::connect().await {
        Ok(db) => match DB.set(db) {
            Ok(()) => {},
            Err(_) => {
                panic!("init_db should only be called once");
            }
        },
        Err(err) => {
            error!("Error while connection to database: {err}");
            exit(1);
        }
    }
}

pub fn get_executor() -> &'static Pool<Sqlite> {
    DB.get().expect("you tried to access the db before it was initialized").get_executor()
}

pub struct Db {
    executor: Pool<Sqlite>
}

impl Db {
    pub async fn connect() -> Result<Db, anyhow::Error> {
        Ok(Db {
            executor: SqlitePoolOptions::new()
                .max_connections(5)
                .connect(&CONFIG.sqlite_url).await?
        })
    }

    pub fn get_executor(&self) -> &Pool<Sqlite> {
        &self.executor
    }
}
