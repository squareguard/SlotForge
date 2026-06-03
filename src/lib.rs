//! SlotForge core library: save discovery, vault, swap/restore, and CLI/desktop API.

pub mod api;
pub mod app;
pub mod domain;
pub mod platform;
pub mod services;
pub mod storage;
pub mod ui;

#[cfg(test)]
mod test_support;

pub use app::run;
