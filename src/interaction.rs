use gtk4::{glib, prelude::*, ApplicationWindow, EventControllerKey, FlowBox, Label, gdk, PropagationPhase};
use std::cell::RefCell;
use std::rc::Rc;
use crate::focus_jumpers::{focus_first_visible, focus_last_visible, focus_line_extremity};

pub(crate) type SearchState = Rc<RefCell<String>>;

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

    controller.connect_key_pressed(move |_, keyval, _, _| {
        let mut is_searching = search_mode_active.borrow_mut();

        if !*is_searching {
            match keyval {
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    handle_selection(&flowbox);
                    return glib::Propagation::Stop;
                }
                gdk::Key::Escape => std::process::exit(0),
                _ => {}
            };
        };

        if vi_mode {
            if *is_searching {
                match keyval {
                    gdk::Key::Escape => {
                        *is_searching = false;
                        clear_search(&query_state, &flowbox, &search_label);
                        focus_first_visible(&flowbox);
                        return glib::Propagation::Stop;
                    }
                    gdk::Key::Return => {
                        *is_searching = false;
                        focus_first_visible(&flowbox);
                        search_label.set_visible(false);
                        return glib::Propagation::Stop;
                    }
                    gdk::Key::BackSpace => {
                        if query_state.borrow().is_empty() {
                            *is_searching = false;
                            clear_search(&query_state, &flowbox, &search_label);
                            if flowbox.selected_children().is_empty() {
                                focus_first_visible(&flowbox);
                            }
                            return glib::Propagation::Stop;
                        }
                    }
                    _ => {}
                };

                // Search Input Processing
                let initial_propagation =
                    handle_search_input(keyval, &query_state, &flowbox, &search_label);

                if *is_searching && query_state.borrow().is_empty() {
                    search_label.set_text("Search: ");
                    search_label.set_visible(true);
                }
                return initial_propagation;
            }

            // Vi Navigation
            if keyval == gdk::Key::g {
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

            match keyval {
                gdk::Key::h => move_focus(gtk4::DirectionType::Left),
                gdk::Key::j => move_focus(gtk4::DirectionType::Down),
                gdk::Key::k => move_focus(gtk4::DirectionType::Up),
                gdk::Key::l => move_focus(gtk4::DirectionType::Right),
                gdk::Key::slash => {
                    *is_searching = true;
                    flowbox.unselect_all();
                    if query_state.borrow().is_empty() {
                        search_label.set_text("Search: ");
                    };
                    search_label.set_visible(true);
                    return glib::Propagation::Stop;
                }
                gdk::Key::G => focus_last_visible(&flowbox),
                gdk::Key::asciicircum /*i.e.: `^`*/ | gdk::Key::caret => {
                    focus_line_extremity(&flowbox, true);
                    return glib::Propagation::Stop;
                }
                gdk::Key::dollar => {
                    focus_line_extremity(&flowbox, false);
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
