use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub aleoapi: Vec<String>,
    pub mysqldsn: String,
    pub batch_request: u32,
    pub batch_concurrent: usize,
    pub address: Vec<String>,
    pub store_block: bool,
    pub listen_ip: String,
}

pub fn load_config() -> Config {
    let config_file = "config.yml";
    let fobj = std::fs::File::open(config_file).unwrap();
    serde_yaml::from_reader(fobj).unwrap()
}