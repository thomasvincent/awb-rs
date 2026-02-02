use gtk::prelude::*;
use gtk::{gio, glib};

pub struct RuleEditor {
    container: gtk::Box,
    list_box: gtk::ListBox,
}

impl RuleEditor {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .vexpand(true)
            .build();

        // Toolbar for rule actions
        let toolbar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(5)
            .margin_start(5)
            .margin_end(5)
            .margin_top(5)
            .margin_bottom(5)
            .build();

        let add_button = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add rule")
            .build();
        toolbar.append(&add_button);

        let remove_button = gtk::Button::builder()
            .icon_name("list-remove-symbolic")
            .tooltip_text("Remove rule")
            .build();
        toolbar.append(&remove_button);

        let up_button = gtk::Button::builder()
            .icon_name("go-up-symbolic")
            .tooltip_text("Move up")
            .build();
        toolbar.append(&up_button);

        let down_button = gtk::Button::builder()
            .icon_name("go-down-symbolic")
            .tooltip_text("Move down")
            .build();
        toolbar.append(&down_button);

        container.append(&toolbar);

        // ListBox for rules
        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();

        scrolled.set_child(Some(&list_box));
        container.append(&scrolled);

        // Connect button actions
        let list_box_weak = list_box.downgrade();
        add_button.connect_clicked(move |_| {
            if let Some(list_box) = list_box_weak.upgrade() {
                Self::add_rule(&list_box);
            }
        });

        let list_box_weak2 = list_box.downgrade();
        remove_button.connect_clicked(move |_| {
            if let Some(list_box) = list_box_weak2.upgrade() {
                if let Some(row) = list_box.selected_row() {
                    list_box.remove(&row);
                }
            }
        });

        Self {
            container,
            list_box,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    fn add_rule(list_box: &gtk::ListBox) {
        let rule_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(5)
            .margin_start(5)
            .margin_end(5)
            .margin_top(5)
            .margin_bottom(5)
            .build();

        let enabled_check = gtk::CheckButton::builder()
            .active(true)
            .build();
        rule_row.append(&enabled_check);

        let pattern_entry = gtk::Entry::builder()
            .placeholder_text("Find pattern...")
            .hexpand(true)
            .build();
        rule_row.append(&pattern_entry);

        let replacement_entry = gtk::Entry::builder()
            .placeholder_text("Replace with...")
            .hexpand(true)
            .build();
        rule_row.append(&replacement_entry);

        let regex_check = gtk::CheckButton::builder()
            .tooltip_text("Use regex")
            .label(".*")
            .build();
        rule_row.append(&regex_check);

        list_box.append(&rule_row);
    }

    pub fn clear_rules(&self) {
        while let Some(row) = self.list_box.first_child() {
            self.list_box.remove(&row);
        }
    }
}

impl Default for RuleEditor {
    fn default() -> Self {
        Self::new()
    }
}
