use config_file::FromConfigFile;
use serde::Deserialize;
use std::env;

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


pub fn init_config_from_file(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    match Config::from_config_file(path){
		Ok(config) => Ok(config),
		Err(e) => Err(Box::new(e)),
	}
}

pub fn init_config_from_str(text: &str) -> Result<Config, Box<dyn std::error::Error>> {
    match toml::from_str(text){
        Ok(config) => Ok(config),
        Err(e) => Err(Box::new(e)),
    }
}

pub async fn init_config() -> Config {
    match env::var_os("APP_NAME"){
        Some(val) => {
            match reqwest::get(format!("http://cc-server.config-center/{}/config.toml/raw", val.into_string().unwrap())).await{
                Ok(res) => {
                    match res.text().await{
                        Ok(text) => {
                            match init_config_from_str(text.as_str()) {
                                Ok(config) => config,
                                Err(e) => {
                                    panic!("Failed to load config: {}", e);
                                },
                            }
                        },
                        Err(e) => {
                            panic!("Failed to load config: {}", e);
                        }
                    }
                },
                Err(e) => {
                    panic!("Failed to load config from config-center: {}", e);
                }
            }
        },
        None => {
            match init_config_from_file("config.toml") {
                Ok(config) => config,
                Err(e) => {
                    panic!("Failed to load config from file: {}", e);
                },
            }
        }
    }
}