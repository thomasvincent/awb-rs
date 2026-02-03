#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod views;

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use gtk::glib;

#[cfg(target_os = "linux")]
fn main() -> glib::ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create the GTK application
    let app = app::AwbApplication::new();

    // Run the application
    app.run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("awb-gtk is only available on Linux systems.");
    eprintln!("This is a GTK4-based UI designed for the Linux desktop.");
    std::process::exit(1);
}
