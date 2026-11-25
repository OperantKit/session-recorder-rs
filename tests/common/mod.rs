//! Helpers shared by the integration tests.
#![allow(dead_code)]

use session_recorder::format::{self, ParsedHeader, ParsedRecord};
use session_recorder::{materialise, MemorySink, OklError, SessionMeta, Value};
use std::path::{Path, PathBuf};

/// Every header and body value of one file, body lines in file order.
pub struct Parsed {
    pub header: ParsedHeader,
    pub records: Vec<ParsedRecord>,
}

/// Parse strictly: a type that the codebook does not declare is an error.
pub fn parse_all(text: &str) -> Result<Parsed, OklError> {
    let lines = format::framed_lines(text);
    let (header, consumed) = format::parse_header(&lines)?;
    let mut records = Vec::new();
    for line in &lines[consumed..] {
        if line.trim().is_empty() {
            continue;
        }
        records.push(format::parse_event_line(line, &header.codebook)?);
    }
    Ok(Parsed { header, records })
}

/// Write the header and records with the file's own codebook.
pub fn write_all(parsed: &Parsed) -> Result<String, OklError> {
    let mut out = Vec::new();
    format::write_header(
        &mut out,
        &parsed.header.session_meta(),
        Some(&parsed.header.codebook),
    )?;
    let mut text = String::from_utf8(out).expect("the writer emits UTF-8");
    for r in &parsed.records {
        text.push_str(&format::encode_record(
            r.timestamp,
            &r.type_name,
            &r.args,
            &parsed.header.codebook,
        )?);
        text.push('\n');
    }
    Ok(text)
}

/// Write through the sink, as a session does: events with `emit`, phases and
/// stimuli with their own methods. Only for files with the canonical codebook.
pub fn write_through_sink(parsed: &Parsed) -> Result<String, OklError> {
    let mut sink = MemorySink::new(Vec::new());
    let meta: SessionMeta = parsed.header.session_meta();
    sink.write_header(&meta)?;
    let text = |r: &ParsedRecord, name: &str| match r.get(name) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    for r in &parsed.records {
        match r.type_name.as_str() {
            "phase_enter" => {
                let label = text(r, "label").unwrap();
                let context = session_recorder::PhaseContext {
                    name: text(r, "name"),
                    time: text(r, "time"),
                    location: text(r, "location"),
                    cue: text(r, "cue"),
                };
                sink.write_phase_enter(r.timestamp, &label, &context)?;
            }
            "phase_exit" => sink.write_phase_exit(r.timestamp, &text(r, "label").unwrap())?,
            "stimulus_on" => sink.write_stimulus_on(r.timestamp, &text(r, "stimulus").unwrap())?,
            "stimulus_off" => {
                sink.write_stimulus_off(r.timestamp, &text(r, "stimulus").unwrap())?
            }
            _ => {
                use session_recorder::EventSink;
                sink.emit(&materialise(r)?)?;
            }
        }
    }
    Ok(sink.into_string())
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// `OKL_CONFORMANCE_ROOT`, or an `OperantKitLog/spec/conformance` checkout next
/// to one of this crate's parent directories.
pub fn conformance_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("OKL_CONFORMANCE_ROOT") {
        return Some(PathBuf::from(root));
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join("OperantKitLog/spec/conformance"))
        .find(|dir| dir.is_dir())
}

pub fn txt_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();
    files
}
