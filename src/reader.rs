use crate::error::OklError;
use crate::events::{SessionEvent, SessionMeta, Value};
use crate::format::{
    self, framed_lines, materialise, parse_event_line, parse_header, Codebook, ParsedRecord,
    PhaseContext,
};
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

/// What to do with a line whose type the file's codebook does not declare.
/// Nothing is printed; skipped types are listed in
/// [`SessionLog::unknown_types`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnUnknown {
    /// Skip the line.
    #[default]
    Skip,
    /// Stop with [`OklError::UnknownType`].
    Error,
}

/// Which end of a phase a [`PhaseMarker`] stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseKind {
    /// The start.
    Enter,
    /// The end.
    Exit,
}

/// The start or end of a phase.
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseMarker {
    /// Start or end.
    pub kind: PhaseKind,
    /// Session time.
    pub timestamp: f64,
    /// Phase label.
    pub label: String,
    /// Context; empty for the end of a phase.
    pub context: PhaseContext,
}

/// Everything read from a log.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionLog {
    /// The header.
    pub meta: SessionMeta,
    /// The file's own event declarations.
    pub codebook: Codebook,
    /// Session events in file order.
    pub events: Vec<SessionEvent>,
    /// Every line except phase markers, in file order: the lines behind
    /// `events`, stimulus presentations and extension types.
    pub raw_records: Vec<ParsedRecord>,
    /// Phase markers in file order.
    pub phase_markers: Vec<PhaseMarker>,
    /// Types that were skipped.
    pub unknown_types: BTreeSet<String>,
    /// Top-level header keys the format does not define; their values are in
    /// `meta.metadata`.
    pub unknown_keys: Vec<String>,
}

/// Read a log from a file. A file that is missing, empty or blank is an
/// empty log.
pub fn read_log(path: impl AsRef<Path>, on_unknown: OnUnknown) -> Result<SessionLog, OklError> {
    let bytes = match std::fs::read(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        other => other?,
    };
    // An interrupted write can stop inside a character, so the incomplete last
    // line is cut off before the bytes are decoded.
    let end = bytes
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |last| last + 1);
    let (complete, torn) = bytes.split_at(end);
    let text = std::str::from_utf8(complete)
        .map_err(|e| OklError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;
    if text.trim().is_empty() && !torn.trim_ascii().is_empty() {
        return Err(OklError::Header("unexpected EOF".to_string()));
    }
    read_str(text, on_unknown)
}

/// Read a log from text. Blank text is an empty log.
pub fn read_str(text: &str, on_unknown: OnUnknown) -> Result<SessionLog, OklError> {
    let mut log = SessionLog {
        meta: SessionMeta::new("", "", 0.0),
        codebook: Codebook::default(),
        events: Vec::new(),
        raw_records: Vec::new(),
        phase_markers: Vec::new(),
        unknown_types: BTreeSet::new(),
        unknown_keys: Vec::new(),
    };
    if text.trim().is_empty() {
        return Ok(log);
    }
    let lines = framed_lines(text);
    let (header, consumed) = parse_header(&lines)?;
    log.meta = header.session_meta();
    for line in &lines[consumed..] {
        if line.trim().is_empty() {
            continue;
        }
        let record = match parse_event_line(line, &header.codebook) {
            Err(OklError::UnknownType(name)) if on_unknown == OnUnknown::Skip => {
                log.unknown_types.insert(name);
                continue;
            }
            other => other?,
        };
        match record.type_name.as_str() {
            format::PHASE_ENTER => log
                .phase_markers
                .push(phase_marker(PhaseKind::Enter, &record)?),
            format::PHASE_EXIT => log
                .phase_markers
                .push(phase_marker(PhaseKind::Exit, &record)?),
            format::RESPONSE
            | format::REINFORCER_START
            | format::REINFORCER_END
            | format::STATE_CHANGE
            | format::COMPONENT_CHANGE => {
                log.events.push(materialise(&record)?);
                log.raw_records.push(record);
            }
            _ => log.raw_records.push(record),
        }
    }
    log.codebook = header.codebook;
    log.unknown_keys = header.unknown_keys;
    Ok(log)
}

fn phase_marker(kind: PhaseKind, record: &ParsedRecord) -> Result<PhaseMarker, OklError> {
    let text = |name: &str| match record.get(name) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let label = text("label").ok_or_else(|| {
        OklError::Record(format!(
            "event {:?}: field \"label\" is missing or has the wrong type",
            record.type_name
        ))
    })?;
    let context = match kind {
        PhaseKind::Enter => PhaseContext {
            name: text("name"),
            time: text("time"),
            location: text("location"),
            cue: text("cue"),
        },
        PhaseKind::Exit => PhaseContext::default(),
    };
    Ok(PhaseMarker {
        kind,
        timestamp: record.timestamp,
        label,
        context,
    })
}
