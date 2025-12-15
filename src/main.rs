use async_channel::Sender;
use gdk_pixbuf::Pixbuf;
use gtk4::{
    gdk, glib, prelude::*, Application, ApplicationWindow, EventControllerKey, FlowBox,
    FlowBoxChild, GestureClick, Label, MovementStep, Overlay, Picture, PropagationPhase,
    ScrolledWindow,
};
use rayon::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use walkdir::WalkDir;

const APP_ID: &str = "eu.soliprem.thumbpick";
const THUMB_SIZE: i32 = 200;
const BATCH_SIZE: usize = 100;

type SearchState = Rc<RefCell<String>>;

fn main() {
    let args: Vec<String> = std::env::args().collect();

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

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, &dir_path, vi_mode));
    app.run_with_args(&Vec::<String>::new());
}

fn build_ui(app: &Application, dir_path: &str, vi_mode: bool) {
    let window = create_main_window(app);
    let flowbox = create_flowbox();
    let scrolled = wrap_in_scroll(&flowbox);

    let (overlay, search_label) = create_search_overlay(&scrolled);
    window.set_child(Some(&overlay));

    let search_query: SearchState = Rc::new(RefCell::new(String::new()));

    setup_filter_func(&flowbox, search_query.clone());

    setup_keyboard_controller(&window, &flowbox, search_query, search_label, vi_mode);

    spawn_image_loader(flowbox, dir_path.to_string());

    window.present();
}

// --- UI Components ---

fn create_main_window(app: &Application) -> ApplicationWindow {
    ApplicationWindow::builder()
        .application(app)
        .title("ThumbPick")
        .default_width(1200)
        .default_height(800)
        .build()
}

fn create_flowbox() -> FlowBox {
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

fn wrap_in_scroll(child: &impl IsA<gtk4::Widget>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_child(Some(child));
    scrolled.set_vexpand(true);
    scrolled
}

fn create_search_overlay(child: &impl IsA<gtk4::Widget>) -> (Overlay, Label) {
    let overlay = Overlay::new();
    overlay.set_child(Some(child));

    let label = Label::new(None);
    label.add_css_class("app-notification");
    label.set_halign(gtk4::Align::Center);
    label.set_valign(gtk4::Align::End);
    label.set_margin_bottom(30);
    label.set_visible(false);

    overlay.add_overlay(&label);
    (overlay, label)
}

// --- Logic & Events ---

fn setup_filter_func(flowbox: &FlowBox, query_state: SearchState) {
    flowbox.set_filter_func(move |child| {
        let query = query_state.borrow();
        if query.is_empty() {
            return true;
        }
        child
            .widget_name()
            .as_str()
            .to_lowercase()
            .contains(&query.to_lowercase())
    });
}

fn setup_keyboard_controller(
    window: &ApplicationWindow,
    flowbox: &FlowBox,
    query_state: SearchState,
    search_label: Label,
    vi_mode: bool,
) {
    let controller = EventControllerKey::new();
    controller.set_propagation_phase(PropagationPhase::Capture);
    let flowbox = flowbox.clone();
    let search_mode_active = Rc::new(RefCell::new(false));

    controller.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
            handle_selection(&flowbox);
            return glib::Propagation::Stop;
        }
        if vi_mode {
            let mut is_searching = search_mode_active.borrow_mut();
            if *is_searching {
                if keyval == gdk::Key::Escape {
                    *is_searching = false;
                    clear_search(&query_state, &flowbox, &search_label);
                    return glib::Propagation::Stop;
                }
                let was_empty = query_state.borrow().is_empty();
                let propagation =
                    handle_search_input(keyval, &query_state, &flowbox, &search_label);
                if was_empty && keyval == gdk::Key::BackSpace {
                    *is_searching = false;
                    clear_search(&query_state, &flowbox, &search_label);
                    if !flowbox
                        .selected_children()
                        .first()
                        .is_some_and(|c| c.grab_focus())
                    {
                        flowbox.grab_focus();
                    }
                    return glib::Propagation::Stop;
                }

                if *is_searching && query_state.borrow().is_empty() {
                    search_label.set_text("Search: ");
                    search_label.set_visible(true);
                }

                return propagation;
            }
            let flowbox_focus = flowbox.clone();
            let move_focus = move |direction: gtk4::DirectionType| {
                if flowbox_focus.selected_children().is_empty() {
                    if let Some(child) = flowbox_focus.child_at_index(0) {
                        child.grab_focus();
                        flowbox_focus.select_child(&child);
                    }
                } else {
                    flowbox_focus.child_focus(direction);
                }
            };
            match keyval {
                gdk::Key::h => move_focus(gtk4::DirectionType::Left),
                gdk::Key::j => move_focus(gtk4::DirectionType::Down),
                gdk::Key::k => move_focus(gtk4::DirectionType::Up),
                gdk::Key::l => move_focus(gtk4::DirectionType::Right),
                gdk::Key::slash => {
                    *is_searching = true;
                    search_label.set_text("Search: ");
                    search_label.set_visible(true);
                    return glib::Propagation::Stop;
                }
                _ => return glib::Propagation::Proceed,
            }
            return glib::Propagation::Stop;
        }

        handle_search_input(keyval, &query_state, &flowbox, &search_label)
    });

    window.add_controller(controller);
}

// --- The "Smart" Helper ---

fn clear_search(query_state: &SearchState, flowbox: &FlowBox, label: &Label) {
    query_state.borrow_mut().clear();
    label.set_visible(false);
    flowbox.invalidate_filter();
}

fn handle_selection(flowbox: &FlowBox) {
    if let Some(child) = flowbox.selected_children().first() {
        println!("{}", child.widget_name());
        std::process::exit(0);
    }
}

// --- Input Handler with UI Feedback ---
fn handle_search_input(
    keyval: gdk::Key,
    query_state: &SearchState,
    flowbox: &FlowBox,
    label: &Label,
) -> glib::Propagation {
    let (should_invalidate, current_text) = {
        let mut query = query_state.borrow_mut();
        let mut updated = false;

        if keyval == gdk::Key::BackSpace {
            query.pop();
            updated = true;
        } else if keyval == gdk::Key::Escape {
            query.clear();
            updated = true;
        } else if let Some(c) = keyval.to_unicode() {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                query.push(c);
                updated = true;
            }
        }
        (updated, query.clone())
    };

    if should_invalidate {
        if current_text.is_empty() {
            label.set_visible(false);
        } else {
            label.set_text(&format!("Search: {}", current_text));
            label.set_visible(true);
        }

        flowbox.invalidate_filter();
        return glib::Propagation::Stop;
    }

    glib::Propagation::Proceed
}

// --- Async Pipeline ---

fn spawn_image_loader(flowbox: FlowBox, dir_path: String) {
    glib::spawn_future_local(async move {
        let (sender, receiver) = async_channel::bounded(10);
        thread::spawn(move || run_scan_and_decode(dir_path, sender));
        while let Ok(thumbnails) = receiver.recv().await {
            for (path, texture) in thumbnails {
                add_thumbnail_to_ui(&flowbox, path, texture);
            }
            glib::timeout_future(std::time::Duration::from_millis(1)).await;
        }
    });
}

fn run_scan_and_decode(dir_path: String, sender: Sender<Vec<(PathBuf, gdk::Texture)>>) {
    let paths = get_file_list(&dir_path);
    for chunk in paths.chunks(BATCH_SIZE) {
        let thumbnails: Vec<_> = chunk
            .par_iter()
            .filter_map(|path| {
                let pixbuf = Pixbuf::from_file_at_scale(path, THUMB_SIZE, THUMB_SIZE, true)
                    .or_else(|_| {
                        // Fallback: load full size and scale with aspect ratio preserved
                        // NOTE: necessary because gifs often break with from_file_at_scale
                        let full = Pixbuf::from_file(path)?;
                        let width = full.width();
                        let height = full.height();
                        let scale = (THUMB_SIZE as f64 / width.max(height) as f64).min(1.0);
                        let new_width = (width as f64 * scale) as i32;
                        let new_height = (height as f64 * scale) as i32;
                        full.scale_simple(new_width, new_height, gdk_pixbuf::InterpType::Bilinear)
                            .ok_or_else(|| {
                                glib::Error::new(glib::FileError::Failed, "Scale failed")
                            })
                    })
                    .ok()?;
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                Some((path.clone(), texture))
            })
            .collect();
        if sender.send_blocking(thumbnails).is_err() {
            break;
        }
    }
}

fn get_file_list(dir_path: &str) -> Vec<PathBuf> {
    WalkDir::new(dir_path)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some(".git"))
        .flatten()
        .map(|e| e.into_path())
        .filter(|p| p.is_file())
        .filter(|p| has_image_extension(p))
        .collect()
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

fn add_thumbnail_to_ui(flowbox: &FlowBox, path: PathBuf, texture: gdk::Texture) {
    let picture = Picture::for_paintable(&texture);
    picture.set_size_request(THUMB_SIZE, THUMB_SIZE);
    picture.set_can_shrink(true);
    picture.set_keep_aspect_ratio(true);

    if let Some(name) = path.to_str() {
        let child = FlowBoxChild::new();
        child.set_widget_name(name);
        let gesture = GestureClick::new();
        let path_string = name.to_string();

        gesture.connect_pressed(move |_, n_press, _, _| {
            if n_press == 2 {
                if let Err(e) = std::process::Command::new("xdg-open")
                    .arg(&path_string)
                    .spawn()
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
    }
}
