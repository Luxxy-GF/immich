use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub db_url: String,
    #[serde(default = "default_media_location")]
    pub media_location: String,
    #[serde(default = "default_redis_hostname")]
    pub redis_hostname: String,
    #[serde(default = "default_redis_port")]
    pub redis_port: u16,
    #[serde(default)]
    pub redis_dbindex: u32,
    #[serde(default)]
    pub redis_username: String,
    #[serde(default)]
    pub redis_password: String,
    #[serde(default)]
    pub redis_socket: String,
    #[serde(default)]
    pub redis_url: String,
}

fn default_port() -> u16 {
    3002
}

fn default_media_location() -> String {
    "/root/immich/upload".to_string()
}

fn default_redis_hostname() -> String {
    "redis".to_string()
}

fn default_redis_port() -> u16 {
    6379
}

pub fn load() -> AppConfig {
    dotenvy::dotenv().ok();
    envy::from_env::<AppConfig>().unwrap_or_else(|_| AppConfig {
        port: default_port(),
        db_url: "".to_string(),
        media_location: default_media_location(),
        redis_hostname: default_redis_hostname(),
        redis_port: default_redis_port(),
        redis_dbindex: 0,
        redis_username: "".to_string(),
        redis_password: "".to_string(),
        redis_socket: "".to_string(),
        redis_url: "".to_string(),
    })
}
