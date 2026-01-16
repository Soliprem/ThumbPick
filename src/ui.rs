use crate::config::Config;
use gtk4::{
    gdk, prelude::*, Application, ApplicationWindow, FlowBox, FlowBoxChild, GestureClick, Label,
    Overlay, Picture, ScrolledWindow,
};
use std::path::PathBuf;

fn open_file_platform(path: &str) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "linux")]
    return std::process::Command::new("xdg-open").arg(path).spawn();
    
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open").arg(path).spawn();
    
    #[cfg(target_os = "windows")]
    return std::process::Command::new("cmd").args(&["/C", "start", path]).spawn();
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Platform not supported"
    ))
}

pub fn create_main_window(app: &Application) -> ApplicationWindow {
    ApplicationWindow::builder()
        .application(app)
        .title("ThumbPick")
        .default_width(1200)
        .default_height(800)
        .build()
}

pub fn create_flowbox() -> FlowBox {
    FlowBox::builder()
        .max_children_per_line(30)
        .selection_mode(gtk4::SelectionMode::Single)
        .row_spacing(10)
        .column_spacing(10)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build()
}

pub fn wrap_in_scroll(child: &impl IsA<gtk4::Widget>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_child(Some(child));
    scrolled.set_vexpand(true);
    scrolled
}

pub fn create_search_overlay(child: &impl IsA<gtk4::Widget>) -> (Overlay, Label) {
    let overlay = Overlay::new();
    overlay.set_child(Some(child));

    let label = Label::new(None);
    label.set_halign(gtk4::Align::Center);
    label.set_valign(gtk4::Align::End);
    label.set_margin_bottom(30);
    label.set_visible(false);

    overlay.add_overlay(&label);
    (overlay, label)
}

pub fn add_thumbnail_to_ui(flowbox: &FlowBox, path: PathBuf, texture: gdk::Texture, vi_mode: bool) {
    let picture = Picture::for_paintable(&texture);
    picture.set_size_request(Config::global().thumb_size, Config::global().thumb_size);
    picture.set_can_shrink(true);
    picture.set_keep_aspect_ratio(true);

    if let Some(name) = path.to_str() {
        let child = FlowBoxChild::new();
        child.set_widget_name(name);
        let gesture = GestureClick::new();
        let path_string = name.to_string();

        gesture.connect_pressed(move |_, n_press, _, _| {
            if n_press == 2 {
                if let Err(e) = open_file_platform(&path_string)
                {
                    eprintln!("Failed to open image: {}", e);
                }
            }
        });
        child.add_controller(gesture);

        let frame = gtk4::Frame::new(None);
        frame.set_child(Some(&picture));
        child.set_child(Some(&frame));
        flowbox.insert(&child, -1);

        if vi_mode && child.index() == 0 {
            child.grab_focus();
            flowbox.select_child(&child);
        }
    }
}
