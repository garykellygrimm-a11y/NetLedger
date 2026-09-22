use std::net::SocketAddr;

use anyhow::{Context, Result};

pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let bind_addr = std::env::var("NETLEDGER_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .context("NETLEDGER_BIND_ADDR must be an address and port, like 127.0.0.1:8080")?;

        Ok(Self {
            database_url,
            bind_addr,
        })
    }
}
