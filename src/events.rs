//! The record types that OKL v1 writes. All times are seconds.
//!
//! The role-named types (`ReinforcerStartEvent`, `ReinforcerEndEvent`,
//! `SessionState::Reinforcing`) come from OKL v1 and are frozen. Records added
//! later carry no theory's role name: a stimulus presentation is written with
//! `write_stimulus_on` / `write_stimulus_off`.

/// Lifecycle states of a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Not started.
    Idle,
    /// Running, no reinforcer being delivered.
    Running,
    /// A reinforcer is being delivered.
    Reinforcing,
    /// Paused.
    Paused,
    /// Ended.
    Finished,
}

impl SessionState {
    /// The spelling used in a log.
    pub fn name(self) -> &'static str {
        match self {
            SessionState::Idle => "IDLE",
            SessionState::Running => "RUNNING",
            SessionState::Reinforcing => "REINFORCING",
            SessionState::Paused => "PAUSED",
            SessionState::Finished => "FINISHED",
        }
    }

    /// The state spelled `name`, if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "IDLE" => SessionState::Idle,
            "RUNNING" => SessionState::Running,
            "REINFORCING" => SessionState::Reinforcing,
            "PAUSED" => SessionState::Paused,
            "FINISHED" => SessionState::Finished,
            _ => return None,
        })
    }
}

/// A single subject response.
///
/// `timestamp` is session time. `device_time` is the time the apparatus
/// itself put on the response; it is kept for the record only.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseEvent {
    /// Response number.
    pub id: i64,
    /// Session time.
    pub timestamp: f64,
    /// Which key or lever, when there is more than one.
    pub operandum: Option<i64>,
    /// The apparatus's own time.
    pub device_time: Option<f64>,
}

/// The start of a reinforcer delivery.
#[derive(Clone, Debug, PartialEq)]
pub struct ReinforcerStartEvent {
    /// Reinforcer number.
    pub id: i64,
    /// Session time.
    pub timestamp: f64,
    /// The operandum whose response produced it.
    pub operandum: Option<i64>,
    /// Strength after discounting by delay; 1.0 when there is none.
    pub potency: f64,
}

/// The end of a reinforcer delivery.
#[derive(Clone, Debug, PartialEq)]
pub struct ReinforcerEndEvent {
    /// Number of the reinforcer that ends.
    pub id: i64,
    /// Session time.
    pub timestamp: f64,
    /// The operandum whose response produced it.
    pub operandum: Option<i64>,
}

/// A change of [`SessionState`].
#[derive(Clone, Debug, PartialEq)]
pub struct StateChangeEvent {
    /// State before.
    pub from_state: SessionState,
    /// State after.
    pub to_state: SessionState,
    /// Session time.
    pub timestamp: f64,
}

/// A change of the component in force (multiple schedules and the like).
///
/// The empty string stands for "before the first" or "after the last".
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentChangeEvent {
    /// Component before.
    pub from_component: String,
    /// Component after.
    pub to_component: String,
    /// Session time.
    pub timestamp: f64,
}

/// Any of the five events a session emits.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    /// See [`ResponseEvent`].
    Response(ResponseEvent),
    /// See [`ReinforcerStartEvent`].
    ReinforcerStart(ReinforcerStartEvent),
    /// See [`ReinforcerEndEvent`].
    ReinforcerEnd(ReinforcerEndEvent),
    /// See [`StateChangeEvent`].
    StateChange(StateChangeEvent),
    /// See [`ComponentChangeEvent`].
    ComponentChange(ComponentChangeEvent),
}

macro_rules! from_event {
    ($($variant:ident($ty:ident)),*) => {$(
        impl From<$ty> for SessionEvent {
            fn from(e: $ty) -> Self {
                SessionEvent::$variant(e)
            }
        }
    )*};
}
from_event!(
    Response(ResponseEvent),
    ReinforcerStart(ReinforcerStartEvent),
    ReinforcerEnd(ReinforcerEndEvent),
    StateChange(StateChangeEvent),
    ComponentChange(ComponentChangeEvent)
);

/// A value of the header's `meta:` section or of a body column.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `int`
    Int(i64),
    /// `float`
    Float(f64),
    /// `str`
    Str(String),
    /// `bool`
    Bool(bool),
}

/// Session metadata, as carried by the header.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionMeta {
    /// Session origin (`session_start`).
    pub timestamp: f64,
    /// Name of the clock behind the timestamps.
    pub clock_type: String,
    /// Session identifier.
    pub session_name: String,
    /// Subject identifier.
    pub subject_id: Option<String>,
    /// Replication number.
    pub replication_index: Option<i64>,
    /// Experiment label.
    pub experiment_name: Option<String>,
    /// Protocol or task version.
    pub protocol_id: Option<String>,
    /// Hash of the source task file.
    pub task_file_hash: Option<String>,
    /// ISO 8601 date and time of the session start, kept as written: it is
    /// neither checked nor respelled.
    pub wall_clock_start: Option<String>,
    /// Free-form notes added after the session.
    pub notes: Vec<String>,
    /// Free-form key/value pairs, in the order they are written.
    pub metadata: Vec<(String, Value)>,
}

impl SessionMeta {
    /// Metadata with the three required fields only.
    pub fn new(
        session_name: impl Into<String>,
        clock_type: impl Into<String>,
        session_start: f64,
    ) -> Self {
        SessionMeta {
            timestamp: session_start,
            clock_type: clock_type.into(),
            session_name: session_name.into(),
            subject_id: None,
            replication_index: None,
            experiment_name: None,
            protocol_id: None,
            task_file_hash: None,
            wall_clock_start: None,
            notes: Vec::new(),
            metadata: Vec::new(),
        }
    }
}
