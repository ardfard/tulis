//! Core logic for Tulis: the document model, editing operations, storage, vim and AI.

pub mod alternates;
pub mod edit;
pub mod model;
pub mod pos;

pub use alternates::{AltInfo, AltLevel, OpError};
pub use model::*;
pub use pos::*;
