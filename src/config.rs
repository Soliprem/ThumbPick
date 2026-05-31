use clap::{ArgAction, Parser};
use directories::ProjectDirs;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Parser, Serialize)]
#[command(
    version,
    about = "Lightweight, scriptable, keyboard-centric image picker",
    author = "soliprem me@soliprem.eu"
)]
struct CliArgs {
    #[arg(long, short = 'v', action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    #[serde(skip_serializing_if = "Option::is_none")]
    vi_mode: Option<bool>,

    #[arg(long, short = 'r', action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    #[serde(skip_serializing_if = "Option::is_none")]
    recursive: Option<bool>,

    #[arg(long, short = 'e', action = ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_error: Option<bool>,

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
    pub recursive: bool,
    pub dir_path: String,
    pub thumb_size: i32,
    pub keys: KeyMap,
    pub exit_error: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vi_mode: false,
            recursive: true,
            dir_path: ".".to_string(),
            thumb_size: 200,
            keys: KeyMap::default(),
            exit_error: true,
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

        let args = CliArgs::parse();
        let builder = build_figment(&args, config_path);

        let mut config: Config = builder.extract().unwrap_or_else(|e| {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        });

        if let Some(expanded) = expand_dir_path(&config.dir_path) {
            config.dir_path = expanded;
        } else {
            // Handle error if a variable is missing (e.g. $INVALID_VAR)
            eprintln!(
                "Warning: Could not expand environment variables in path: {}",
                config.dir_path
            );
        }

        if let Err(error) = validate_dir_path(&config.dir_path) {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }

        CONFIG.set(config).ok();
        CONFIG.get().unwrap()
    }

    pub fn global() -> &'static Config {
        CONFIG.get().expect("Config is not initialized")
    }
}

fn build_figment(args: &CliArgs, config_path: Option<PathBuf>) -> Figment {
    let mut builder = Figment::new();

    builder = builder.merge(Serialized::defaults(Config::default()));

    if let Some(path) = config_path {
        builder = builder.merge(Toml::file(path));
    }

    builder = builder.merge(Env::prefixed("THUMBPICK_").split("__"));

    builder.merge(Serialized::defaults(args))
}

fn expand_dir_path(path: &str) -> Option<String> {
    shellexpand::full(path).ok().map(|path| path.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigPathError {
    DoesNotExist(String),
    NotDirectory(String),
}

impl std::fmt::Display for ConfigPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigPathError::DoesNotExist(path) => {
                write!(f, "Directory '{path}' does not exist")
            }
            ConfigPathError::NotDirectory(path) => write!(f, "'{path}' is not a directory"),
        }
    }
}

fn validate_dir_path(path: &str) -> Result<(), ConfigPathError> {
    let path_ref = Path::new(path);
    if !path_ref.exists() {
        return Err(ConfigPathError::DoesNotExist(path.to_string()));
    }
    if !path_ref.is_dir() {
        return Err(ConfigPathError::NotDirectory(path.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        saved: Vec<(String, Option<OsString>)>,
    }

    impl EnvGuard {
        fn clear_prefixed(prefix: &str) -> Self {
            let saved = std::env::vars_os()
                .filter_map(|(key, value)| {
                    let key = key.into_string().ok()?;
                    key.starts_with(prefix).then_some((key, Some(value)))
                })
                .collect::<Vec<_>>();

            for (key, _) in &saved {
                std::env::remove_var(key);
            }

            Self { saved }
        }

        fn set(&mut self, key: &str, value: &str) {
            if !self.saved.iter().any(|(saved_key, _)| saved_key == key) {
                self.saved.push((key.to_string(), std::env::var_os(key)));
            }
            std::env::set_var(key, value);
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn empty_args() -> CliArgs {
        CliArgs {
            vi_mode: None,
            recursive: None,
            exit_error: None,
            dir_path: None,
            thumb_size: None,
        }
    }

    fn temp_config(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!(
            "thumbpick-config-test-{}-{suffix}.toml",
            std::process::id()
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let mut path = std::env::temp_dir();
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            path.push(format!(
                "thumbpick-config-dir-test-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn extract_config(args: &CliArgs, config_path: Option<PathBuf>) -> Config {
        build_figment(args, config_path).extract().unwrap()
    }

    #[test]
    fn defaults_are_used_without_file_env_or_cli_overrides() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::clear_prefixed("THUMBPICK_");

        let config = extract_config(&empty_args(), None);

        assert!(!config.vi_mode);
        assert!(config.recursive);
        assert_eq!(config.dir_path, ".");
        assert_eq!(config.thumb_size, 200);
        assert_eq!(config.keys.up, "k");
        assert_eq!(config.keys.search, "slash");
        assert!(config.exit_error);
    }

    #[test]
    fn config_file_overrides_defaults_without_replacing_unspecified_keys() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::clear_prefixed("THUMBPICK_");
        let config_path = temp_config(
            r#"
vi_mode = true
recursive = false
thumb_size = 96

[keys]
up = "w"
"#,
        );

        let config = extract_config(&empty_args(), Some(config_path.clone()));
        fs::remove_file(config_path).unwrap();

        assert!(config.vi_mode);
        assert!(!config.recursive);
        assert_eq!(config.thumb_size, 96);
        assert_eq!(config.keys.up, "w");
        assert_eq!(config.keys.down, "j");
    }

    #[test]
    fn double_underscore_env_overrides_nested_key_config() {
        let _guard = ENV_LOCK.lock().unwrap();
        let mut env = EnvGuard::clear_prefixed("THUMBPICK_");
        env.set("THUMBPICK_KEYS__UP", "t");

        let config = extract_config(&empty_args(), None);

        assert_eq!(config.keys.up, "t");
    }

    #[test]
    fn env_overrides_config_file_and_cli_overrides_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let mut env = EnvGuard::clear_prefixed("THUMBPICK_");
        env.set("THUMBPICK_THUMB_SIZE", "128");
        env.set("THUMBPICK_KEYS__UP", "e");
        let config_path = temp_config(
            r#"
thumb_size = 64

[keys]
up = "c"
"#,
        );
        let mut args = empty_args();
        args.thumb_size = Some(256);

        let config = extract_config(&args, Some(config_path.clone()));
        fs::remove_file(config_path).unwrap();

        assert_eq!(config.thumb_size, 256);
        assert_eq!(config.keys.up, "e");
    }

    #[test]
    fn cli_args_parse_optional_boolean_flags_and_positional_dir() {
        let args = CliArgs::parse_from([
            "thumbpick",
            "--vi-mode=false",
            "-r",
            "--exit-error=false",
            "--thumb-size",
            "144",
            "/tmp",
        ]);

        assert_eq!(args.vi_mode, Some(false));
        assert_eq!(args.recursive, Some(true));
        assert_eq!(args.exit_error, Some(false));
        assert_eq!(args.thumb_size, Some(144));
        assert_eq!(args.dir_path, Some("/tmp".to_string()));
    }

    #[test]
    fn path_validation_accepts_dirs_and_rejects_missing_paths_and_files() {
        let temp = TempDir::new();
        let file = temp.path().join("file.txt");
        fs::write(&file, b"text").unwrap();
        let missing = temp.path().join("missing");

        assert_eq!(validate_dir_path(temp.path().to_str().unwrap()), Ok(()));
        assert_eq!(
            validate_dir_path(file.to_str().unwrap()),
            Err(ConfigPathError::NotDirectory(
                file.to_str().unwrap().to_string()
            ))
        );
        assert_eq!(
            validate_dir_path(missing.to_str().unwrap()),
            Err(ConfigPathError::DoesNotExist(
                missing.to_str().unwrap().to_string()
            ))
        );
    }

    #[test]
    fn dir_path_expands_environment_variables() {
        let _guard = ENV_LOCK.lock().unwrap();
        let mut env = EnvGuard::clear_prefixed("THUMBPICK_TEST_");
        env.set("THUMBPICK_TEST_DIR", "/tmp/thumbpick-test");

        assert_eq!(
            expand_dir_path("$THUMBPICK_TEST_DIR/images"),
            Some("/tmp/thumbpick-test/images".to_string())
        );
        assert_eq!(expand_dir_path("$THUMBPICK_TEST_MISSING/images"), None);
    }
}
