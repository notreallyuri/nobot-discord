use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[source] Box<sqlx::Error>),
    #[error("migration error: {0}")]
    Migrate(#[source] Box<sqlx::migrate::MigrateError>),
    #[error("discord error: {0}")]
    Serenity(#[source] Box<serenity::Error>),
    #[error("http error: {0}")]
    Http(#[source] Box<reqwest::Error>),
    #[error("{0}")]
    Message(String),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(Box::new(e))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        Self::Migrate(Box::new(e))
    }
}

impl From<serenity::Error> for AppError {
    fn from(e: serenity::Error) -> Self {
        Self::Serenity(Box::new(e))
    }
}
