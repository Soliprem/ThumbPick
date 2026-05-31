use gtk4::{
    gdk, glib, prelude::*, ApplicationWindow, EventControllerKey, FlowBox, Label, PropagationPhase,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::config::{Config, KeyMap};
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
        Self {
            map: build_key_map(&Config::global().keys),
        }
    }

    fn resolve(&self, keyval: gdk::Key) -> Action {
        resolve_action(&self.map, keyval)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchUpdate {
    changed: bool,
    current_text: String,
}

fn build_key_map(keys: &KeyMap) -> HashMap<gdk::Key, Action> {
    let mut map = HashMap::new();

    let mut bind = |name: &str, action: Action| {
        if let Some(key) = gdk::Key::from_name(name) {
            map.insert(key, action);
        }
    };

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

    map.insert(gdk::Key::KP_Enter, Action::Select);

    map
}

fn resolve_action(map: &HashMap<gdk::Key, Action>, keyval: gdk::Key) -> Action {
    map.get(&keyval).copied().unwrap_or(Action::None)
}

fn update_search_query(query: &mut String, keyval: gdk::Key) -> SearchUpdate {
    let mut changed = false;

    if keyval == gdk::Key::BackSpace {
        query.pop();
        changed = true;
    } else if keyval == gdk::Key::Escape {
        query.clear();
        changed = true;
    } else if let Some(c) = keyval.to_unicode() {
        if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
            query.push(c);
            changed = true;
        }
    }

    SearchUpdate {
        changed,
        current_text: query.clone(),
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
    let update = {
        let mut query = query_state.borrow_mut();
        update_search_query(&mut query, keyval)
    };

    if update.changed {
        if update.current_text.is_empty() {
            label.set_visible(false);
        } else {
            label.set_text(&format!("Search: {}", update.current_text));
            label.set_visible(true);
        }

        flowbox.invalidate_filter();
        return glib::Propagation::Stop;
    }

    glib::Propagation::Proceed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_map_resolves_configured_actions_and_ignores_invalid_key_names() {
        let keys = KeyMap {
            up: "w".to_string(),
            down: "not-a-real-gdk-key".to_string(),
            ..Default::default()
        };

        let map = build_key_map(&keys);

        assert_eq!(resolve_action(&map, gdk::Key::w), Action::Up);
        assert_eq!(resolve_action(&map, gdk::Key::j), Action::None);
        assert_eq!(resolve_action(&map, gdk::Key::KP_Enter), Action::Select);
    }

    #[test]
    fn search_query_accepts_expected_printable_characters() {
        let mut query = String::new();

        for key in [
            gdk::Key::a,
            gdk::Key::_1,
            gdk::Key::minus,
            gdk::Key::underscore,
            gdk::Key::period,
            gdk::Key::space,
        ] {
            assert!(update_search_query(&mut query, key).changed);
        }

        assert_eq!(query, "a1-_. ");
    }

    #[test]
    fn search_query_handles_backspace_escape_and_unsupported_keys() {
        let mut query = "abc".to_string();

        let update = update_search_query(&mut query, gdk::Key::BackSpace);
        assert!(update.changed);
        assert_eq!(update.current_text, "ab");

        let update = update_search_query(&mut query, gdk::Key::Left);
        assert!(!update.changed);
        assert_eq!(update.current_text, "ab");

        let update = update_search_query(&mut query, gdk::Key::Escape);
        assert!(update.changed);
        assert_eq!(update.current_text, "");
    }
}
