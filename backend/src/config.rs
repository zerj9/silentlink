use dotenvy::dotenv;
use sqlx::PgPool;
use std::env;
use std::num::ParseIntError;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub max_connections: u32,
    pub graph_name: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingVar(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if it exists
        dotenv().ok();

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::MissingVar("DATABASE_URL".to_string()))?;

        let max_connections = env::var("PG_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())
            .parse::<u32>()
            .map_err(|e: ParseIntError| {
                ConfigError::InvalidValue("PG_MAX_CONNECTIONS".to_string(), e.to_string())
            })?;

        let graph_name = env::var("GRAPH_NAME")
            .map_err(|_| ConfigError::MissingVar("GRAPH_NAME".to_string()))?;

        Ok(Config {
            database_url,
            max_connections,
            graph_name,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<PgPool>,
    pub graph_name: String,
}
