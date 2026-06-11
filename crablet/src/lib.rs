#![deny(unused_imports)]
#![allow(dead_code)]
#![allow(unknown_lints)]
// P0 Safety: Deny unwrap() in non-test code to prevent production panics.
// Test code can still use unwrap() via #[allow(clippy::unwrap_used)] on individual test fns.
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

mod sqlx_compat;
pub use sqlx_compat::*;
extern crate self as sqlx;

pub mod agent;
pub mod channels;
pub mod cognitive;
pub mod config;
pub mod constants;
pub mod error;
pub mod events;
#[cfg(feature = "knowledge")]
pub mod knowledge;
pub mod memory;
pub mod observability;
pub mod plugins;
pub mod safety;
pub mod sandbox;
#[cfg(feature = "scripting")]
pub mod scripting;
pub mod skills;
pub mod storage;
pub mod tools;
pub mod types;
pub mod workflow;

pub mod audit;
#[cfg(feature = "web")]
pub mod auth;
pub mod background;
#[cfg(feature = "web")]
pub mod gateway;
pub mod health;
pub mod heartbeat;
pub mod protocols;
pub mod telemetry;
pub mod testing;

// Auto-Working & RPA modules
#[cfg(feature = "auto-working")]
pub mod auto_working;
#[cfg(feature = "auto-working")]
pub mod connectors;
#[cfg(feature = "auto-working")]
pub mod rpa;
