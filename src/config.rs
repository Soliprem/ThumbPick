use std::env;

pub struct Config {
    pub dir_path: String,
    pub vi_mode: bool,
}

impl Config {
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut dir_path = String::new();
        let mut vi_mode = false;

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--vi" | "--vi-mode" => vi_mode = true,
                path if !path.starts_with("--") => dir_path = path.to_string(),
                _ => {}
            }
        }

        if dir_path.is_empty() {
            eprintln!("Usage: thumbpick <directory> [--vi | --vi-mode]");
            std::process::exit(1);
        }

        Self { dir_path, vi_mode }
    }
}
