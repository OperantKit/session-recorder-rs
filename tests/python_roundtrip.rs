//! Logs written by the Python session-recorder (`tests/fixtures/okl/`, see
//! SOURCES.md there).

mod common;

use common::{fixture, parse_all, write_through_sink};
use session_recorder::{read_log, OnUnknown, SessionEvent};

const FILES: [&str; 3] = ["fr3", "conc", "phases"];

#[test]
fn rewriting_a_python_log_gives_the_same_bytes() {
    for name in FILES {
        let text = std::fs::read_to_string(fixture(&format!("okl/{name}.txt"))).unwrap();
        let written = write_through_sink(&parse_all(&text).unwrap()).unwrap();
        assert_eq!(written, text, "{name}");
    }
}

#[test]
fn fr3_has_the_events_of_a_fixed_ratio_session() {
    let log = read_log(fixture("okl/fr3.txt"), OnUnknown::Error).unwrap();
    assert_eq!(log.meta.session_name, "session");
    assert_eq!(log.meta.clock_type, "ManualClock");
    let responses = log
        .events
        .iter()
        .filter(|e| matches!(e, SessionEvent::Response(_)))
        .count();
    let reinforcers = log
        .events
        .iter()
        .filter(|e| matches!(e, SessionEvent::ReinforcerStart(_)))
        .count();
    assert_eq!((responses, reinforcers), (15, 5));
}

#[test]
fn conc_keeps_the_operandum() {
    let log = read_log(fixture("okl/conc.txt"), OnUnknown::Error).unwrap();
    let mut seen = [0usize; 2];
    for e in &log.events {
        if let SessionEvent::Response(r) = e {
            seen[r.operandum.unwrap() as usize] += 1;
        }
    }
    assert!(seen[0] > 0 && seen[1] > 0, "{seen:?}");
}

#[test]
fn phases_and_stimuli_stay_out_of_events() {
    let log = read_log(fixture("okl/phases.txt"), OnUnknown::Error).unwrap();
    assert_eq!(log.events.len(), 153);
    let labels: Vec<_> = log
        .phase_markers
        .iter()
        .map(|p| (p.kind, p.label.as_str()))
        .collect();
    use session_recorder::PhaseKind::{Enter, Exit};
    assert_eq!(
        labels,
        [
            (Enter, "acquisition"),
            (Exit, "acquisition"),
            (Enter, "extinction"),
            (Exit, "extinction"),
            (Enter, "test"),
            (Exit, "test"),
        ]
    );
    assert_eq!(log.phase_markers[0].context.name.as_deref(), Some("A"));
    assert_eq!(log.phase_markers[0].context.cue, None);
    let stimuli = log
        .raw_records
        .iter()
        .filter(|r| r.type_name.starts_with("stimulus_"))
        .count();
    assert_eq!(stimuli, 4);
    assert_eq!(log.raw_records.len(), 153 + 4);
}
