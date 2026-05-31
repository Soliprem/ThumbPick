use gtk4::{
    gdk, glib, prelude::*, ApplicationWindow, EventControllerKey, FlowBox, Label, PropagationPhase,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::config::Config;
use crate::focus_jumpers::{focus_first_visible, focus_last_visible, focus_line_extremity};

pub(crate) type SearchState = Rc<RefCell<String>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Left,
    Down,
    Up,
    Right,
    Search,
    Quit,
    Select,
    GoTop,
    GoBottom,
    LineStart,
    LineEnd,
    None,
}

struct KeyResolver {
    map: HashMap<gdk::Key, Action>,
}

impl KeyResolver {
    fn new() -> Self {
        let keys = &Config::global().keys;
        let mut map = HashMap::new();

        // Helper to bind a config string to an action
        let mut bind = |name: &str, action: Action| {
            if let Some(key) = gdk::Key::from_name(name) {
                map.insert(key, action);
            }
        };

        // Bind from Config
        bind(&keys.left, Action::Left);
        bind(&keys.down, Action::Down);
        bind(&keys.up, Action::Up);
        bind(&keys.right, Action::Right);
        bind(&keys.search, Action::Search);
        bind(&keys.quit, Action::Quit);
        bind(&keys.select, Action::Select);
        bind(&keys.go_top, Action::GoTop);
        bind(&keys.go_bottom, Action::GoBottom);
        bind(&keys.line_start, Action::LineStart);
        bind(&keys.line_end, Action::LineEnd);

        // Hardcode secondary defaults if desired (e.g. Numpad Enter always works)
        map.insert(gdk::Key::KP_Enter, Action::Select);

        Self { map }
    }

    fn resolve(&self, keyval: gdk::Key) -> Action {
        self.map.get(&keyval).copied().unwrap_or(Action::None)
    }
}

pub fn setup_filter_func(flowbox: &FlowBox, query_state: SearchState) {
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

pub fn setup_keyboard_controller(
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
    let awaiting_g = Rc::new(RefCell::new(false));

    let resolver = KeyResolver::new();

    controller.connect_key_pressed(move |_, keyval, _, _| {
        let mut is_searching = search_mode_active.borrow_mut();

        let action = resolver.resolve(keyval);

        if !*is_searching {
            match action {
                Action::Select => {
                    handle_selection(&flowbox);
                    return glib::Propagation::Stop;
                }
                Action::Quit => std::process::exit(Config::global().exit_error as i32),
                _ => {}
            }
        };

        if vi_mode {
            if *is_searching {
                match action {
                    Action::Quit => {
                        *is_searching = false;
                        clear_search(&query_state, &flowbox, &search_label);
                        focus_first_visible(&flowbox);
                        return glib::Propagation::Stop;
                    }
                    Action::Select => {
                        *is_searching = false;
                        focus_first_visible(&flowbox);
                        search_label.set_visible(false);
                        return glib::Propagation::Stop;
                    }
                    _ => {
                        if keyval == gdk::Key::BackSpace && query_state.borrow().is_empty() {
                            *is_searching = false;
                            clear_search(&query_state, &flowbox, &search_label);
                            if flowbox.selected_children().is_empty() {
                                focus_first_visible(&flowbox);
                            }
                            return glib::Propagation::Stop;
                        }
                    }
                }

                let initial_propagation =
                    handle_search_input(keyval, &query_state, &flowbox, &search_label);

                if *is_searching && query_state.borrow().is_empty() {
                    search_label.set_text("Search: ");
                    search_label.set_visible(true);
                }
                return initial_propagation;
            }

            // --- Navigation Mode ---

            // Handle double-press logic (gg)
            if action == Action::GoTop {
                let mut g_layer = awaiting_g.borrow_mut();
                if *g_layer {
                    focus_first_visible(&flowbox);
                    *g_layer = false;
                } else {
                    *g_layer = true;
                }
                return glib::Propagation::Stop;
            }
            if *awaiting_g.borrow() {
                *awaiting_g.borrow_mut() = false;
            }

            let flowbox_focus = flowbox.clone();
            let move_focus = move |direction: gtk4::DirectionType| {
                if flowbox_focus.selected_children().is_empty() {
                    focus_first_visible(&flowbox_focus);
                } else {
                    flowbox_focus.child_focus(direction);
                }
            };

            // Main Action Matcher
            match action {
                Action::Left => move_focus(gtk4::DirectionType::Left),
                Action::Down => move_focus(gtk4::DirectionType::Down),
                Action::Up => move_focus(gtk4::DirectionType::Up),
                Action::Right => move_focus(gtk4::DirectionType::Right),
                Action::Search => {
                    *is_searching = true;
                    flowbox.unselect_all();
                    if query_state.borrow().is_empty() {
                        search_label.set_text("Search: ");
                    };
                    search_label.set_visible(true);
                }
                Action::GoBottom => focus_last_visible(&flowbox),
                Action::LineStart => focus_line_extremity(&flowbox, true),
                Action::LineEnd => focus_line_extremity(&flowbox, false),
                _ => return glib::Propagation::Proceed,
            }
            return glib::Propagation::Stop;
        }

        handle_search_input(keyval, &query_state, &flowbox, &search_label)
    });

    window.add_controller(controller);
}

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
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
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
