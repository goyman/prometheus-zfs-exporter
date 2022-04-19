use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Snapshot {
    pub name: String,
    #[serde(rename(deserialize = "match"))]
    pub match_string: String,
}

#[derive(Deserialize, Debug)]
pub struct Dataset {
    pub name: String,
    pub snapshots: Vec<String>,
    #[serde(default)]
    pub recurse: bool,
}

#[derive(Deserialize, Debug)]
pub struct Pool {
    pub name: String,
}

fn default_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 9150)
}

fn default_true() -> bool {
    true
}

fn default_interval() -> u64 {
    60_000
}

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "Vec::new")]
    pub snapshots: Vec<Snapshot>,

    #[serde(default = "Vec::new")]
    pub datasets: Vec<Dataset>,

    #[serde(default = "Vec::new")]
    pub pools: Vec<Pool>,

    #[serde(default = "default_addr")]
    pub listen: SocketAddr,

    #[serde(default = "default_true")]
    pub all_pools: bool,

    #[serde(default = "default_true")]
    pub all_datasets: bool,

    #[serde(default = "default_interval")]
    pub interval: u64,

}

impl Config {
    pub fn read(path: &Path) -> Result<Self> {
        let config_data = fs::read(&path).with_context(|| {
            format!("Cannot read config file at `{}`", path.display())
        })?;
        let config_data = String::from_utf8_lossy(&config_data);

        toml::from_str(&config_data).with_context(|| {
            format!("Cannot decode TOML file `{}`", path.display())
        })
    }
}
