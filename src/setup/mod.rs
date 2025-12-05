//! Browser-based credential setup flow for first-time configuration.
//!
//! This module provides a lightweight local HTTP server that serves an HTML form
//! for configuring Trakt API credentials when `credentials.ini` is missing or incomplete.

mod html;
mod server;

pub use server::{run_setup_server, SetupResult};
