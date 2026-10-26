pub mod error;
pub mod events;
#[cfg(test)]
mod ipc_contract;
pub mod paths;
pub mod prevent_default;
#[cfg(target_os = "windows")]
pub mod windows_args;

pub use error::{AppError, Result};
