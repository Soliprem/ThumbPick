use clap::{ArgAction, Parser};
use directories::ProjectDirs;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Parser, Serialize)]
struct CliArgs {
    #[arg(long, short = 'v', action = ArgAction::SetTrue)]
    #[serde(skip_serializing_if = "Option::is_none")]
    vi_mode: Option<bool>,

    #[arg(index = 1)]
    #[serde(skip_serializing_if = "Option::is_none")]
    dir_path: Option<String>,

    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    thumb_size: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub vi_mode: bool,
    pub dir_path: String,
    pub thumb_size: i32,
    pub keys: KeyMap,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vi_mode: false,
            dir_path: ".".to_string(),
            thumb_size: 200,
            keys: KeyMap::default(),
        }
    }
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
    pub go_top: String,
    pub go_bottom: String,
    pub line_start: String,
    pub line_end: String,
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
            line_start: "asciicircum".to_string(),
            line_end: "dollar".to_string(),
        }
    }
}

impl Config {
    pub fn parse() -> &'static Self {
        let app_name = "thumbpick";
        let config_path = ProjectDirs::from("eu", "soliprem", app_name)
            .map(|dirs| dirs.config_dir().join("config.toml"));

        // Parse args into a mutable variable so we can mutate vi_mode later
        let mut args = CliArgs::parse();

        // If vi_mode is Some(false), it means the flag was missing (default).
        // Set it to None so Serde skips it and Figment uses the TOML value.
        // This assumes you only use the flag to ENABLE vi_mode, not disable it.
        if args.vi_mode == Some(false) {
            args.vi_mode = None;
        }

        let mut builder = Figment::new();

        builder = builder.merge(Serialized::defaults(Config::default()));

        if let Some(path) = config_path {
            println!("Looking for config at: {:?}", path);
            if path.exists() {
                builder = builder.merge(Toml::file(path));
            }
        }

        builder = builder.merge(Env::prefixed("THUMBPICK_"));

        builder = builder.merge(Serialized::defaults(&args));

        let mut config: Config = builder.extract().unwrap_or_else(|e| {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        });

        if let Ok(expanded) = shellexpand::full(&config.dir_path) {
            config.dir_path = expanded.to_string();
        } else {
             // Optional: Handle error if a variable is missing (e.g. $INVALID_VAR)
             eprintln!("Warning: Could not expand environment variables in path: {}", config.dir_path);
        }

        CONFIG.set(config).ok();
        CONFIG.get().unwrap()
    }

    pub fn global() -> &'static Config {
        CONFIG.get().expect("Config is not initialized")
    }
}
