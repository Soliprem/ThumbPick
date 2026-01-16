use clap::{ArgAction, Parser};
use directories::ProjectDirs;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Deserialize, Serialize, Parser)]
#[command(author, version, about)]
pub struct Config {
    #[arg(long, short = 'v', action = ArgAction::SetTrue)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vi_mode: Option<bool>,
    #[arg(default_value = ".")]
    pub dir_path: String,
    #[arg(long, default_value = "200")]
    pub thumb_size: i32,
    #[arg(skip)]
    #[serde(default)]
    pub keys: KeyMap,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KeyMap {
    pub left: String,
    pub down: String,
    pub up: String,
    pub right: String,
    pub search: String,
    pub quit: String,
    pub select: String,
    pub go_top: String,      // 'g' (triggers on double-tap)
    pub go_bottom: String,   // 'G'
    pub line_start: String,  // '^'
    pub line_end: String,    // '$'
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            left: "h".to_string(),
            down: "j".to_string(),
            up: "k".to_string(),
            right: "l".to_string(),
            search: "slash".to_string(),
            quit: "Escape".to_string(),
            select: "Return".to_string(),
            go_top: "g".to_string(),
            go_bottom: "G".to_string(),
            line_start: "asciicircum".to_string(), // GDK name for '^'
            line_end: "dollar".to_string(),
        }
    }
}

impl Config {
    pub fn parse() -> &'static Self {
        let app_name = "thumbpick";
        let config_path = ProjectDirs::from("eu", "soliprem", app_name)
            .map(|dirs| dirs.config_dir().join("config.toml"));

        let args = Config::parse_from(std::env::args());

        let mut builder = Figment::new();

        if let Some(path) = config_path {
            builder = builder.merge(Toml::file(path));
        }

        builder = builder.merge(Env::prefixed("THUMBPICK_"));
        builder = builder.merge(Serialized::defaults(&args));

        let config: Config = builder.extract().unwrap_or_else(|e| {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        });

        CONFIG.set(config).ok();
        CONFIG.get().unwrap()
    }

    pub fn global() -> &'static Config {
        CONFIG.get().expect("Config is not initialized")
    }
}
