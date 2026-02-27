use gtk::prelude::*;

pub struct EditorView {
    container: gtk::Box,
    #[allow(dead_code)]
    source_view: gtk::TextView,
    #[allow(dead_code)]
    diff_view: gtk::TextView,
    #[allow(dead_code)]
    notebook: gtk::Notebook,
}

impl EditorView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .vexpand(true)
            .hexpand(true)
            .build();

        // Create notebook for tabs
        let notebook = gtk::Notebook::builder().vexpand(true).hexpand(true).build();

        // Source view tab
        let source_view = gtk::TextView::builder()
            .monospace(true)
            .wrap_mode(gtk::WrapMode::None)
            .left_margin(5)
            .right_margin(5)
            .top_margin(5)
            .bottom_margin(5)
            .build();

        let source_scroll = gtk::ScrolledWindow::builder()
            .child(&source_view)
            .vexpand(true)
            .hexpand(true)
            .build();

        notebook.append_page(&source_scroll, Some(&gtk::Label::new(Some("Source"))));

        // Diff view tab (split pane)
        let diff_paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Vertical)
            .wide_handle(true)
            .build();

        let before_view = gtk::TextView::builder()
            .monospace(true)
            .editable(false)
            .wrap_mode(gtk::WrapMode::None)
            .left_margin(5)
            .right_margin(5)
            .top_margin(5)
            .bottom_margin(5)
            .build();

        let before_scroll = gtk::ScrolledWindow::builder()
            .child(&before_view)
            .vexpand(true)
            .hexpand(true)
            .build();

        let before_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        before_box.append(&gtk::Label::new(Some("Before (Original)")));
        before_box.append(&before_scroll);
        diff_paned.set_start_child(Some(&before_box));

        let after_view = gtk::TextView::builder()
            .monospace(true)
            .editable(false)
            .wrap_mode(gtk::WrapMode::None)
            .left_margin(5)
            .right_margin(5)
            .top_margin(5)
            .bottom_margin(5)
            .build();

        let after_scroll = gtk::ScrolledWindow::builder()
            .child(&after_view)
            .vexpand(true)
            .hexpand(true)
            .build();

        let after_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        after_box.append(&gtk::Label::new(Some("After (Modified)")));
        after_box.append(&after_scroll);
        diff_paned.set_end_child(Some(&after_box));

        notebook.append_page(&diff_paned, Some(&gtk::Label::new(Some("Diff View"))));

        container.append(&notebook);

        Self {
            container,
            source_view,
            diff_view: after_view,
            notebook,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    #[allow(dead_code)]
    pub fn set_text(&self, text: &str) {
        if let Ok(buffer) = self.source_view.buffer().downcast::<gtk::TextBuffer>() {
            buffer.set_text(text);
        }
    }

    #[allow(dead_code)]
    pub fn get_text(&self) -> String {
        if let Ok(buffer) = self.source_view.buffer().downcast::<gtk::TextBuffer>() {
            let start = buffer.start_iter();
            let end = buffer.end_iter();
            buffer.text(&start, &end, false).to_string()
        } else {
            String::new()
        }
    }

    #[allow(dead_code)]
    pub fn set_diff(&self, before: &str, after: &str) {
        // TODO: Implement proper diff highlighting
        if let Ok(buffer) = self.diff_view.buffer().downcast::<gtk::TextBuffer>() {
            buffer.set_text(&format!(
                "=== BEFORE ===\n{}\n\n=== AFTER ===\n{}",
                before, after
            ));
        }
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.set_text("");
        self.set_diff("", "");
    }
}

impl Default for EditorView {
    fn default() -> Self {
        Self::new()
    }
}
