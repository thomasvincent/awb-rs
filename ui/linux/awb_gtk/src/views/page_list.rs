use gtk::prelude::*;
use gtk::{gio, glib};

pub struct PageList {
    container: gtk::Box,
    list_box: gtk::ListBox,
    search_entry: gtk::SearchEntry,
}

impl PageList {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .vexpand(true)
            .build();

        // Search bar
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Search pages...")
            .margin_start(5)
            .margin_end(5)
            .margin_top(5)
            .margin_bottom(5)
            .build();
        container.append(&search_entry);

        // Scrolled window with list
        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();

        scrolled.set_child(Some(&list_box));
        container.append(&scrolled);

        // Connect search functionality
        let list_box_weak = list_box.downgrade();
        search_entry.connect_search_changed(move |entry| {
            if let Some(list_box) = list_box_weak.upgrade() {
                let query = entry.text().to_lowercase();
                Self::filter_list(&list_box, &query);
            }
        });

        Self {
            container,
            list_box,
            search_entry,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn add_page(&self, title: &str) {
        let row = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .margin_start(10)
            .margin_end(10)
            .margin_top(5)
            .margin_bottom(5)
            .build();

        self.list_box.append(&row);
    }

    pub fn clear_pages(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
    }

    fn filter_list(list_box: &gtk::ListBox, query: &str) {
        if query.is_empty() {
            list_box.set_filter_func(None::<Box<dyn Fn(&gtk::ListBoxRow) -> bool>>);
        } else {
            let query = query.to_string();
            list_box.set_filter_func(Some(Box::new(move |row| {
                if let Some(child) = row.child() {
                    if let Ok(label) = child.downcast::<gtk::Label>() {
                        let text = label.text().to_lowercase();
                        return text.contains(&query);
                    }
                }
                false
            })));
        }
    }
}

impl Default for PageList {
    fn default() -> Self {
        Self::new()
    }
}
