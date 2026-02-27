use adw::prelude::*;
use gtk::prelude::*;
use gtk::{gio, glib};
use libadwaita as adw;

use super::editor_view::EditorView;
use super::login_dialog::LoginDialog;
use super::page_list::PageList;
use super::rule_editor::RuleEditor;

pub struct MainWindow {
    window: adw::ApplicationWindow,
    _page_list: PageList,
    _editor: EditorView,
    _rule_editor: RuleEditor,
    #[allow(dead_code)]
    status_label: gtk::Label,
    #[allow(dead_code)]
    progress_bar: gtk::ProgressBar,
}

impl MainWindow {
    pub fn new(app: &adw::Application) -> Self {
        // Create the main window
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("AWBrowser - AutoWikiBrowser Rust Edition")
            .default_width(1400)
            .default_height(800)
            .build();

        // Create header bar with menu button
        let header_bar = adw::HeaderBar::new();

        // Create menu button
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .build();

        // Build the menu
        let menu = gio::Menu::new();
        let file_menu = gio::Menu::new();
        file_menu.append(Some("Login..."), Some("app.login"));
        file_menu.append(Some("Quit"), Some("app.quit"));
        menu.append_submenu(Some("File"), &file_menu);

        let edit_menu = gio::Menu::new();
        edit_menu.append(Some("Apply Rules"), Some("app.apply_rules"));
        edit_menu.append(Some("Preview Changes"), Some("app.preview"));
        menu.append_submenu(Some("Edit"), &edit_menu);

        let help_menu = gio::Menu::new();
        help_menu.append(Some("About"), Some("app.about"));
        menu.append_submenu(Some("Help"), &help_menu);

        menu_button.set_menu_model(Some(&menu));
        header_bar.pack_end(&menu_button);

        // Create toolbar buttons
        let login_button = gtk::Button::builder()
            .icon_name("system-users-symbolic")
            .tooltip_text("Login to wiki")
            .build();
        header_bar.pack_start(&login_button);

        // Create main content layout (three-panel)
        let main_paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .wide_handle(true)
            .build();

        // Left panel: Page list
        let page_list = PageList::new();
        let left_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(300)
            .build();
        left_box.append(&gtk::Label::new(Some("Pages")));
        left_box.append(page_list.widget());
        main_paned.set_start_child(Some(&left_box));

        // Center + Right: Editor and Rules
        let center_right_paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .wide_handle(true)
            .build();

        // Center: Editor
        let editor = EditorView::new();
        let editor_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();
        editor_box.append(&gtk::Label::new(Some("Editor")));
        editor_box.append(editor.widget());
        center_right_paned.set_start_child(Some(&editor_box));

        // Right: Rule editor
        let rule_editor = RuleEditor::new();
        let right_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(300)
            .build();
        right_box.append(&gtk::Label::new(Some("Rules")));
        right_box.append(rule_editor.widget());
        center_right_paned.set_end_child(Some(&right_box));

        main_paned.set_end_child(Some(&center_right_paned));

        // Create status bar at bottom
        let status_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_start(10)
            .margin_end(10)
            .margin_top(5)
            .margin_bottom(5)
            .build();

        let status_label = gtk::Label::builder()
            .label("Ready")
            .hexpand(true)
            .xalign(0.0)
            .build();
        status_bar.append(&status_label);

        let page_count_label = gtk::Label::new(Some("0 pages"));
        status_bar.append(&page_count_label);

        let progress_bar = gtk::ProgressBar::builder().width_request(150).build();
        status_bar.append(&progress_bar);

        // Assemble the window
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&main_paned));
        toolbar_view.add_bottom_bar(&status_bar);

        content_box.append(&toolbar_view);
        window.set_content(Some(&content_box));

        // Set up actions
        let window_weak = window.downgrade();
        let login_action = gio::SimpleAction::new("login", None);
        login_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                Self::show_login_dialog(&window);
            }
        });
        app.add_action(&login_action);

        let quit_action = gio::SimpleAction::new("quit", None);
        quit_action.connect_activate(glib::clone!(
            #[weak]
            app,
            move |_, _| {
                app.quit();
            }
        ));
        app.add_action(&quit_action);

        let about_action = gio::SimpleAction::new("about", None);
        about_action.connect_activate(glib::clone!(
            #[weak]
            window,
            move |_, _| {
                Self::show_about_dialog(&window);
            }
        ));
        app.add_action(&about_action);

        // Connect login button
        login_button.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| {
                Self::show_login_dialog(&window);
            }
        ));

        Self {
            window,
            _page_list: page_list,
            _editor: editor,
            _rule_editor: rule_editor,
            status_label,
            progress_bar,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }

    #[allow(dead_code)]
    pub fn set_status(&self, message: &str) {
        self.status_label.set_label(message);
    }

    #[allow(dead_code)]
    pub fn set_progress(&self, fraction: f64) {
        self.progress_bar.set_fraction(fraction);
    }

    fn show_login_dialog(parent: &adw::ApplicationWindow) {
        let dialog = LoginDialog::new(parent);
        dialog.present();
    }

    fn show_about_dialog(parent: &adw::ApplicationWindow) {
        let about = adw::AboutDialog::builder()
            .application_name("AWBrowser")
            .application_icon("text-editor-symbolic")
            .developer_name("AWB-RS Team")
            .version("0.1.0")
            .license_type(gtk::License::MitX11)
            .website("https://github.com/thomasvincent/awb-rs")
            .issue_url("https://github.com/thomasvincent/awb-rs/issues")
            .comments("A modern WikiText editor built with Rust and GTK4")
            .build();

        about.present(Some(parent));
    }
}
