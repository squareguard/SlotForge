//! SlotForge core library: save discovery, vault, swap/restore, and CLI/desktop API.

pub mod api;
pub mod app;
pub mod domain;
pub mod platform;
pub mod services;
pub mod storage;
pub mod ui;

pub use app::run;
