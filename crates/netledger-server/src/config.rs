use std::net::SocketAddr;

use ipnet::IpNet;

use anyhow::{Context, Result};

use crate::passwords::HashAlgorithm;

pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub password_hash: HashAlgorithm,
    pub cookie_secure: bool,
    pub trusted_proxies: Vec<IpNet>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let bind_addr = std::env::var("NETLEDGER_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .context("NETLEDGER_BIND_ADDR must be an address and port, like 127.0.0.1:8080")?;
        let password_hash = std::env::var("NETLEDGER_PASSWORD_HASH")
            .ok()
            .map(|value| HashAlgorithm::parse(&value))
            .unwrap_or(Some(HashAlgorithm::Argon2id))
            .context("NETLEDGER_PASSWORD_HASH must be argon2id or pbkdf2-sha256")?;

        let cookie_secure = match std::env::var("NETLEDGER_COOKIE_SECURE").as_deref() {
            Ok("false") => false,
            Ok("true") | Err(_) => true,
            Ok(_) => anyhow::bail!("NETLEDGER_COOKIE_SECURE must be true or false"),
        };

        let trusted_proxies = std::env::var("NETLEDGER_TRUSTED_PROXIES")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                entry
                    .parse::<IpNet>()
                    .or_else(|_| entry.parse::<std::net::IpAddr>().map(IpNet::from))
                    .with_context(|| {
                        format!(
                            "NETLEDGER_TRUSTED_PROXIES contains an invalid address or CIDR: {entry}"
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            database_url,
            bind_addr,
            password_hash,
            cookie_secure,
            trusted_proxies,
        })
    }
}
