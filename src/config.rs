use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub aleoapi: Vec<String>,
    pub mysqldns: String
}

pub fn load_config() -> Config {
    let config_file = "config.yml";
    let fobj = std::fs::File::open(config_file).unwrap();
    serde_yaml::from_reader(fobj).unwrap()
}