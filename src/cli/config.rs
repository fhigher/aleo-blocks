use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub aleoapi: Vec<String>,
    pub mysqldsn: String,
    pub batch_request: u32,
    pub batch_concurrent: usize,
    pub address: Vec<String>,
    pub store_block: bool,
    pub synced_height_file: String,
    pub listen_ip: String,
}

pub fn load_config(path: String) -> Config {
    let fobj = std::fs::File::open(path).unwrap();
    serde_yaml::from_reader(fobj).unwrap()
}