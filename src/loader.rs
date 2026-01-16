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
    std::thread::spawn(move || {
        let walker = WalkDir::new(dir_path).into_iter();
        for entry in walker
            .filter_entry(|e| e.file_name().to_str() != Some(".git"))
            .flatten()
        {
            let path = entry.into_path();
            if path.is_file() && has_image_extension(&path) {
                // If receiver hangs up, stop scanning
                if path_tx.send(path).is_err() {
                    return;
                }
            }
        }
    });
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    while let Ok(path) = path_rx.recv() {
        batch.push(path);

        if batch.len() >= BATCH_SIZE {
            let chunk = std::mem::take(&mut batch);
            process_and_send_chunk(chunk, &sender);
        }
    }
    if !batch.is_empty() {
        process_and_send_chunk(batch, &sender);
    }
}

fn process_and_send_chunk(chunk: Vec<PathBuf>, sender: &Sender<Vec<(PathBuf, gdk::Texture)>>) {
    let thumbnails: Vec<_> = chunk
        .par_iter()
        .filter_map(|path| {
            let pixbuf = Pixbuf::from_file_at_scale(
                path,
                Config::global().thumb_size,
                Config::global().thumb_size,
                true,
            )
            .or_else(|_| {
                // Fallback: load full size and scale with aspect ratio preserved
                // NOTE: necessary because gifs often break with from_file_at_scale
                let full = Pixbuf::from_file(path)?;
                let width = full.width();
                let height = full.height();
                let scale =
                    (Config::global().thumb_size as f64 / width.max(height) as f64).min(1.0);
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
