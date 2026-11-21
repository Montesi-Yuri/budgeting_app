//! The budgeting app's core: database, single write path with sync log,
//! hybrid logical clock and domain logic. It has no UI or Tauri dependency,
//! so it is shared by the desktop and Android builds and tested on its own.

pub mod accounts;
pub mod date;
mod db;
pub mod error;
pub mod hlc;
pub mod settings;
pub mod store;
pub mod sync;

pub use db::SCHEMA_VERSION;
pub use error::{Error, Result};
pub use store::Store;
