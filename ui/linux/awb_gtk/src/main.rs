mod app;
mod views;

use gtk::prelude::*;
use gtk::glib;

fn main() -> glib::ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create the GTK application
    let app = app::AwbApplication::new();

    // Run the application
    app.run()
}
