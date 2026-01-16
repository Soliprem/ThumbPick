use gtk4::{prelude::*, FlowBox, FlowBoxChild};

pub fn focus_first_visible(flowbox: &FlowBox) {
    let mut current = flowbox.first_child();
    while let Some(widget) = current {
        let next = widget.next_sibling();
        if widget.is_visible() && widget.is_sensitive() && widget.is_child_visible() {
            if let Ok(child) = widget.downcast::<FlowBoxChild>() {
                child.grab_focus();
                flowbox.select_child(&child);
                return;
            }
        }
        current = next;
    }
}

pub fn focus_last_visible(flowbox: &FlowBox) {
    let mut current = flowbox.last_child();
    while let Some(widget) = current {
        let next = widget.prev_sibling();
        if widget.is_visible() && widget.is_sensitive() && widget.is_child_visible() {
            if let Ok(child) = widget.downcast::<FlowBoxChild>() {
                child.grab_focus();
                flowbox.select_child(&child);
                return;
            }
        }
        current = next;
    }
}

pub fn focus_line_extremity(flowbox: &FlowBox, start: bool) {
    let current_selection = flowbox.selected_children();
    let current_child = match current_selection.first() {
        Some(w) => w,
        None => return,
    };
    let current_bounds = current_child
        .compute_bounds(flowbox)
        .expect("Widget not in flowbox?");
    let target_y = current_bounds.y();
    let mut candidate = current_child.clone();
    let mut iterator = current_child.clone();
    loop {
        let next_step = if start {
            iterator.prev_sibling()
        } else {
            iterator.next_sibling()
        };

match next_step {
            Some(widget) => {
                if let Ok(child) = widget.downcast::<gtk4::FlowBoxChild>() {
                    iterator = child;
                } else {
                    break;
                }
                if !iterator.is_visible() || !iterator.is_sensitive() || !iterator.is_child_visible() {
                    continue;
                }
                if let Some(bounds) = iterator.compute_bounds(flowbox) {
                    if (bounds.y() - target_y).abs() > 1.0 {
                        break;
                    }
                }
                candidate = iterator.clone();
            }
            None => break,
        }
    }

    candidate.grab_focus();
    flowbox.select_child(&candidate);
}

