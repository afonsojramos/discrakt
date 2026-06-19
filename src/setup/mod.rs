//! Browser-based credential setup flow for first-time configuration.
//!
//! This module runs a lightweight local HTTP server that serves the setup
//! wizard (a Vite/React app embedded from `setup-ui/dist`) and exposes the JSON
//! endpoints it calls, used when no source is configured yet.

mod server;

pub use server::{run_setup_server, SetupResult};
