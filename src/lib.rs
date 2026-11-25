//! OKL v1 (OperantKitLog v1) in Rust: the record types, the event sink trait,
//! and the writer and reader of the plain-text format.
//!
//! The same files are written and read by the Python `session-recorder`; the
//! format itself is specified by OperantKitLog. Running a session is not this
//! crate's job, and it depends on no session engine.
//!
//! ```
//! use session_recorder::{
//!     read_str, EventSink, MemorySink, OnUnknown, ResponseEvent, SessionMeta,
//! };
//!
//! # fn main() -> Result<(), session_recorder::OklError> {
//! let mut sink = MemorySink::new(Vec::new());
//! sink.write_header(&SessionMeta::new("demo", "ManualClock", 0.0))?;
//! sink.emit(&ResponseEvent { id: 1, timestamp: 0.5, operandum: None, device_time: None }.into())?;
//!
//! let log = read_str(&sink.into_string(), OnUnknown::Error)?;
//! assert_eq!(log.events.len(), 1);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod error;
pub mod events;
pub mod format;
mod reader;
mod sink;

pub use error::OklError;
pub use events::{
    ComponentChangeEvent, ReinforcerEndEvent, ReinforcerStartEvent, ResponseEvent, SessionEvent,
    SessionMeta, SessionState, StateChangeEvent, Value,
};
pub use format::{encode_event, materialise, PhaseContext};
pub use reader::{read_log, read_str, OnUnknown, PhaseKind, PhaseMarker, SessionLog};
pub use sink::{EventSink, MemorySink, OklSink, OklWriter};
