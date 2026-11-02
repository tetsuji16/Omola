//! UI building blocks. Each file adds `impl OmolaApp` methods for one area of the window:
//!
//! - `chrome.rs`: menu bar, ribbon tabs + toolbar, status bar, messages window
//! - `browsers.rs`: package/component browser (modeling) and variable browser (simulation)
//! - `views.rs`: edit window (Text/Diagram/Icon/Documentation layers) and plot window
//! - `dialogs.rs`: Simulation Setup and path dialogs

mod browsers;
mod chrome;
mod dialogs;
mod views;
