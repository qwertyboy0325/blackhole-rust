//! Gate 1B0 experimental contract — shared experiment schema and analytic systems.
//! Not a production integration abstraction.

#![forbid(unsafe_code)]

pub mod determinism;
pub mod digest;
pub mod event;
pub mod schema;
pub mod systems;

pub use determinism::{repeat_in_process, RepeatSummary};
pub use digest::json_digest;
pub use event::{localize_event, EventLocalizationResult};
pub use schema::*;
pub use systems::*;
