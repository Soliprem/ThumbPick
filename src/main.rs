mod config;
mod focus_jumpers;
mod interaction;
mod loader;
mod ui;
use crate::config::Config;
use crate::interaction::{setup_filter_func, setup_keyboard_controller, SearchState};
use crate::loader::spawn_image_loader;
use crate::ui::{create_flowbox, create_main_window, create_search_overlay, wrap_in_scroll};
use gtk4::{prelude::*, Application};
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "eu.soliprem.thumbpick";

fn main() {
    let config = Config::parse();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        build_ui(app, &config.dir_path, config.vi_mode.unwrap_or(false))
    });
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

    spawn_image_loader(flowbox, dir_path.to_string(), vi_mode);

    window.present();
}
