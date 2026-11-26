//! RustPocket server library.
//!
//! The binary in `main.rs` only wires the modules together; everything that
//! deserves a test lives here.

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod limiter;
pub mod sandbox;
pub mod state;
