use gtk::prelude::*;
use gtk::glib;
use libadwaita as adw;
use adw::prelude::*;

pub struct LoginDialog {
    dialog: adw::Dialog,
    wiki_url_entry: adw::EntryRow,
    username_entry: adw::EntryRow,
    password_entry: adw::PasswordEntryRow,
}

impl LoginDialog {
    pub fn new(parent: &adw::ApplicationWindow) -> Self {
        // Create preference groups
        let preferences_group = adw::PreferencesGroup::builder()
            .title("Wiki Connection")
            .description("Enter your MediaWiki credentials")
            .build();

        // Wiki URL entry
        let wiki_url_entry = adw::EntryRow::builder()
            .title("Wiki URL")
            .text("https://en.wikipedia.org/w/api.php")
            .build();
        preferences_group.add(&wiki_url_entry);

        // Username entry
        let username_entry = adw::EntryRow::builder()
            .title("Username")
            .build();
        preferences_group.add(&username_entry);

        // Password entry
        let password_entry = adw::PasswordEntryRow::builder()
            .title("Password")
            .build();
        preferences_group.add(&password_entry);

        // Create preferences page
        let preferences_page = adw::PreferencesPage::new();
        preferences_page.add(&preferences_group);

        // Create the dialog
        let dialog = adw::Dialog::builder()
            .title("Login to Wiki")
            .content_width(450.0)
            .content_height(400.0)
            .build();

        // Create toolbar view for dialog content
        let header_bar = adw::HeaderBar::new();

        let cancel_button = gtk::Button::builder()
            .label("Cancel")
            .build();
        header_bar.pack_start(&cancel_button);

        let login_button = gtk::Button::builder()
            .label("Login")
            .css_classes(vec!["suggested-action"])
            .build();
        header_bar.pack_end(&login_button);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&preferences_page));

        dialog.set_child(Some(&toolbar_view));

        // Connect buttons
        let dialog_weak = dialog.downgrade();
        cancel_button.connect_clicked(move |_| {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.close();
            }
        });

        let dialog_weak2 = dialog.downgrade();
        let wiki_url_weak = wiki_url_entry.downgrade();
        let username_weak = username_entry.downgrade();
        let password_weak = password_entry.downgrade();

        login_button.connect_clicked(move |_| {
            let wiki_url = wiki_url_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let username = username_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let password = password_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();

            // Validate inputs
            if wiki_url.is_empty() || username.is_empty() || password.is_empty() {
                // TODO: Show error toast
                tracing::warn!("Login validation failed: missing required fields");
                return;
            }

            // TODO: Implement actual login via FFI
            tracing::info!("Login attempted for user {} at {}", username, wiki_url);

            if let Some(dialog) = dialog_weak2.upgrade() {
                dialog.close();
            }
        });

        Self {
            dialog,
            wiki_url_entry,
            username_entry,
            password_entry,
        }
    }

    pub fn present(&self) {
        self.dialog.present(None::<&adw::ApplicationWindow>);
    }

    pub fn wiki_url(&self) -> String {
        self.wiki_url_entry.text().to_string()
    }

    pub fn username(&self) -> String {
        self.username_entry.text().to_string()
    }

    pub fn password(&self) -> String {
        self.password_entry.text().to_string()
    }
}
