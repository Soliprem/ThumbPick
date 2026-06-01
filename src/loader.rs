use crate::config::Config;
use crate::ui::add_thumbnail_to_ui;
use async_channel::Sender;
use gdk_pixbuf::Pixbuf;
use gtk4::{gdk, glib, FlowBox};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use walkdir::WalkDir;
const BATCH_SIZE: usize = 100;

pub fn spawn_image_loader(flowbox: FlowBox, dir_path: String, vi_mode: bool) {
    glib::spawn_future_local(async move {
        let (sender, receiver) = async_channel::bounded(10);
        thread::spawn(move || run_scan_and_decode(dir_path, sender));
        while let Ok(thumbnails) = receiver.recv().await {
            for (path, texture) in thumbnails {
                add_thumbnail_to_ui(&flowbox, path, texture, vi_mode);
            }
            glib::timeout_future(std::time::Duration::from_millis(1)).await;
        }
    });
}

fn run_scan_and_decode(dir_path: String, sender: Sender<Vec<(PathBuf, gdk::Texture)>>) {
    let (path_tx, path_rx) = mpsc::sync_channel::<PathBuf>(1024);
    let recursive = Config::global().recursive;
    std::thread::spawn(move || {
        for path in image_paths(dir_path, recursive) {
            // If receiver hangs up, stop scanning
            if path_tx.send(path).is_err() {
                return;
            }
        }
    });
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    while let Ok(path) = path_rx.recv() {
        if let Some(chunk) = push_path_batch(&mut batch, path, BATCH_SIZE) {
            process_and_send_chunk(chunk, &sender);
        }
    }
    if let Some(chunk) = finish_path_batch(batch) {
        process_and_send_chunk(chunk, &sender);
    }
}

fn process_and_send_chunk(chunk: Vec<PathBuf>, sender: &Sender<Vec<(PathBuf, gdk::Texture)>>) {
    let thumbnails: Vec<_> = chunk
        .par_iter()
        .filter_map(|path| {
            let pixbuf = Pixbuf::from_file_at_scale(
                path,
                Config::global().size,
                Config::global().size,
                true,
            )
            .or_else(|_| {
                // Fallback: load full size and scale with aspect ratio preserved
                // NOTE: necessary because gifs often break with from_file_at_scale
                let full = Pixbuf::from_file(path)?;
                let width = full.width();
                let height = full.height();
                let scale = (Config::global().size as f64 / width.max(height) as f64).min(1.0);
                let new_width = (width as f64 * scale) as i32;
                let new_height = (height as f64 * scale) as i32;
                full.scale_simple(new_width, new_height, gdk_pixbuf::InterpType::Bilinear)
                    .ok_or_else(|| glib::Error::new(glib::FileError::Failed, "Scale failed"))
            })
            .ok()?;
            let texture = gdk::Texture::for_pixbuf(&pixbuf);
            Some((path.clone(), texture))
        })
        .collect();

    let _ = sender.send_blocking(thumbnails);
}

fn push_path_batch(
    batch: &mut Vec<PathBuf>,
    path: PathBuf,
    batch_size: usize,
) -> Option<Vec<PathBuf>> {
    batch.push(path);
    if batch.len() >= batch_size {
        Some(std::mem::take(batch))
    } else {
        None
    }
}

fn finish_path_batch(batch: Vec<PathBuf>) -> Option<Vec<PathBuf>> {
    if !batch.is_empty() {
        Some(batch)
    } else {
        None
    }
}

fn image_paths(dir_path: impl AsRef<Path>, recursive: bool) -> impl Iterator<Item = PathBuf> {
    let max_depth = if recursive { usize::MAX } else { 1 };

    WalkDir::new(dir_path)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some(".git"))
        .flatten()
        .filter_map(|entry| {
            let path = entry.into_path();
            (path.is_file() && has_image_extension(&path)).then_some(path)
        })
}

fn has_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
                "thumbpick-loader-test-{}-{suffix}",
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

    #[test]
    fn image_extension_matching_is_case_insensitive() {
        assert!(has_image_extension(Path::new("photo.JPG")));
        assert!(has_image_extension(Path::new("photo.webp")));
        assert!(!has_image_extension(Path::new("photo.txt")));
        assert!(!has_image_extension(Path::new("photo")));
    }

    #[test]
    fn image_paths_respects_recursive_mode_and_ignores_git_dir() {
        let temp = TempDir::new();
        let nested = temp.path().join("nested");
        let git = temp.path().join(".git");
        fs::create_dir(&nested).unwrap();
        fs::create_dir(&git).unwrap();
        fs::write(temp.path().join("root.png"), b"not really an image").unwrap();
        fs::write(temp.path().join("notes.txt"), b"text").unwrap();
        fs::write(nested.join("child.jpg"), b"not really an image").unwrap();
        fs::write(git.join("ignored.png"), b"not really an image").unwrap();

        let flat = image_paths(temp.path(), false).collect::<Vec<_>>();
        assert_eq!(flat, vec![temp.path().join("root.png")]);

        let recursive = image_paths(temp.path(), true).collect::<Vec<_>>();
        assert!(recursive.contains(&temp.path().join("root.png")));
        assert!(recursive.contains(&nested.join("child.jpg")));
        assert!(!recursive.contains(&git.join("ignored.png")));
        assert!(!recursive.contains(&temp.path().join("notes.txt")));
    }

    #[test]
    fn path_batch_helpers_preserve_order_and_include_remainder() {
        let paths = ["one", "two", "three", "four", "five"]
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let mut batch = Vec::with_capacity(2);
        let mut batches = Vec::new();

        for path in paths {
            if let Some(batch) = push_path_batch(&mut batch, path, 2) {
                batches.push(batch);
            }
        }
        if let Some(batch) = finish_path_batch(batch) {
            batches.push(batch);
        }

        assert_eq!(
            batches,
            vec![
                vec![PathBuf::from("one"), PathBuf::from("two")],
                vec![PathBuf::from("three"), PathBuf::from("four")],
                vec![PathBuf::from("five")],
            ]
        );
    }

    #[test]
    fn finish_path_batch_omits_empty_batch() {
        assert_eq!(finish_path_batch(Vec::new()), None);
    }
}
