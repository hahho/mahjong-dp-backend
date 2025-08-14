// Mahjong types and metrics
pub mod types;
pub mod hand;

// Re-export commonly used types from types module
pub use types::{Tile, Dimension, Metrics, NUM_ROUNDS};

// Re-export everything from hand module for backward compatibility
pub use hand::*; 