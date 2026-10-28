//! Core logic for Tulis: the document model, editing operations, storage, vim and AI.

pub mod alternates;
pub mod config;
pub mod edit;
pub mod ghost;
pub mod history;
pub mod model;
pub mod motion;
pub mod overflow;
pub mod pos;
pub mod storage;

pub use alternates::{AltInfo, AltLevel, OpError};
pub use model::*;
pub use pos::*;
