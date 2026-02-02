use gtk::prelude::*;
use gtk::{gio, glib, Application};
use libadwaita as adw;
use adw::prelude::*;

use crate::views::main_window::MainWindow;

const APP_ID: &str = "org.awb_rs.AWBrowser";

pub struct AwbApplication {
    app: adw::Application,
}

impl AwbApplication {
    pub fn new() -> Self {
        // Create the application
        let app = adw::Application::builder()
            .application_id(APP_ID)
            .flags(gio::ApplicationFlags::default())
            .build();

        // Connect startup signal
        app.connect_startup(|_| {
            tracing::info!("AWB GTK application starting up");
        });

        // Connect activate signal
        app.connect_activate(Self::on_activate);

        Self { app }
    }

    fn on_activate(app: &adw::Application) {
        tracing::info!("Application activated");

        // Create and present the main window
        let window = MainWindow::new(app);
        window.present();
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

impl Default for AwbApplication {
    fn default() -> Self {
        Self::new()
    }
}
