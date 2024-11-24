use config_file::FromConfigFile;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub db: Database,
    pub redis: Redis,
    pub tags_db: TagsDB,
    pub untagged_db: UntaggedDB,
}

#[derive(Deserialize, Clone)]
pub struct Database {
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct Redis {
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct TagsDB {
    pub url: String,
}
#[derive(Deserialize, Clone)]
pub struct UntaggedDB {
    pub url: String,
}


pub fn init_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    match Config::from_config_file(path){
		Ok(config) => Ok(config),
		Err(e) => Err(Box::new(e)),
	}
}