#![deny(unused_imports)]
#![allow(dead_code)]
#![warn(clippy::unwrap_used)] // Warn on unwrap usage instead of deny for now

pub mod cognitive;
pub mod memory;
pub mod safety;
pub mod tools;
pub mod channels;
pub mod config;
pub mod constants;
pub mod types;
#[cfg(feature = "scripting")]
pub mod scripting;
#[cfg(feature = "knowledge")]
pub mod knowledge;
pub mod skills;
pub mod events;
pub mod plugins;
pub mod agent;
pub mod error;
pub mod sandbox;

pub mod heartbeat;
pub mod background;
pub mod audit;
pub mod auth;
pub mod health;
#[cfg(feature = "web")]
pub mod gateway;
pub mod telemetry;
pub mod protocols;
pub mod testing;

// Auto-Working & RPA modules
#[cfg(feature = "auto-working")]
pub mod auto_working;
#[cfg(feature = "auto-working")]
pub mod rpa;
#[cfg(feature = "auto-working")]
pub mod connectors;
