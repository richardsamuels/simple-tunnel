use crate::Result;
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use snafu::prelude::*;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::vec::Vec;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(deserialize_with = "de_psk")]
    pub psk: String,
    pub addr: String,
    pub transport: crate::config::server::Transport,
    #[serde(default = "default_mtu", deserialize_with = "warn_mtu")]
    pub mtu: u16,
    #[serde(deserialize_with = "de_tunnels")]
    pub tunnels: HashMap<u16, Tunnel>,
    pub crypto: Option<CryptoConfig>,
}

fn de_tunnels<'de, D>(deserializer: D) -> std::result::Result<HashMap<u16, Tunnel>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let tunnels: Vec<Tunnel> = Vec::<Tunnel>::deserialize(deserializer)?;
    let tunnel_map: HashMap<_, _> = tunnels.into_iter().map(|c| (c.remote_port, c)).collect();
    Ok(tunnel_map)
}

fn de_psk<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let psk: String = String::deserialize(deserializer)?;
    if psk.is_empty() || psk.len() > 512 {
        return Err(serde::de::Error::custom(
            "psk must be non-empty and at most 512 bytes long",
        ));
    }
    Ok(psk)
}

fn default_mtu() -> u16 {
    1500
}

fn warn_mtu<'de, D>(deserializer: D) -> std::result::Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u16::deserialize(deserializer)?;
    eprintln!("Warning: mtu parameter is currently ignored.");
    Ok(value)
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CryptoConfig {
    #[serde(default = "localhost_ipv4", deserialize_with = "de_sni_name")]
    pub sni_name: String,
    #[serde(deserialize_with = "de_ca_file")]
    pub ca: Option<PathBuf>,
}

fn de_sni_name<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let sni_name: String = String::deserialize(deserializer)?;
    if rustls::pki_types::ServerName::try_from(sni_name.clone()).is_err() {
        return Err(serde::de::Error::custom(
            "sni_name invalid. expected IP address or hostname",
        ));
    }
    Ok(sni_name)
}

fn de_ca_file<'de, D>(deserializer: D) -> std::result::Result<Option<PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let path: Option<PathBuf> = Option::<PathBuf>::deserialize(deserializer)?;
    if let Some(ref p) = path {
        if !p.exists() {
            return Err(serde::de::Error::custom(format!(
                "CA file does not exist: {:?}",
                p
            )));
        }
    }
    Ok(path)
}

fn localhost_ipv4() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tunnel {
    pub remote_port: u16,
    #[serde(default = "localhost_ipv4")]
    pub local_hostname: String,
    pub local_port: u16,
}

#[derive(Debug)]
pub struct Crypto {
    pub ca: Vec<CertificateDer<'static>>,
}

impl Crypto {
    pub fn from_config(cfg: &CryptoConfig) -> Result<Crypto> {
        Self::new(&cfg.ca)
    }

    fn new<P: AsRef<Path> + std::fmt::Debug>(ca_file: &Option<P>) -> Result<Crypto> {
        use rustls_pki_types::{pem::PemObject, CertificateDer};

        let ca: Vec<_> = match ca_file {
            None => Vec::new(),
            Some(ca_file) => CertificateDer::pem_file_iter(ca_file)
                .with_context(|_| crate::config::CertificateSnafu {})?
                .filter_map(|x| x.ok())
                .collect(),
        };
        Ok(Crypto { ca })
    }
}

pub fn load_config(config: &Path) -> crate::config::Result<Config> {
    let config_contents = read_to_string(config).with_context(|_| crate::config::IoSnafu {
        message: format!("failed to read config file '{:?}'", config),
    })?;

    let c: Config =
        toml::from_str(&config_contents).with_context(|_| crate::config::DecodeSnafu {})?;
    Ok(c)
}
