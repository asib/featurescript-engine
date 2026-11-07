//! A FeatureScript interpreter that runs Onshape's std unchanged.

// Map keys are `Value`s, and boxes inside them have interior mutability. Boxes order by their creation number,
// which never changes, so keys never move.
#![allow(clippy::mutable_key_type)]

pub mod module;
pub mod types;
pub mod value;

pub use value::Value;
