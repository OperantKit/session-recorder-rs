//! The wire format, the sink and the reader, case by case.

mod common;

use session_recorder::format::{
    canonical_codebook, decode_str, encode_record, encode_str, parse_event_line, parse_float,
    parse_header, parse_int, Codebook,
};
use session_recorder::{
    encode_event, read_log, read_str, ComponentChangeEvent, EventSink, MemorySink, OklError,
    OklSink, OnUnknown, PhaseContext, ReinforcerEndEvent, ReinforcerStartEvent, ResponseEvent,
    SessionEvent, SessionMeta, SessionState, StateChangeEvent, Value,
};

const HEAD: &str =
    "# OKL v1\n# session_name = \"s\"\n# clock_type = \"c\"\n# session_start = 0.0\n";

fn header(extra: &str) -> Result<session_recorder::format::ParsedHeader, OklError> {
    let text = format!("{HEAD}{extra}# ---\n");
    let lines = session_recorder::format::framed_lines(&text);
    parse_header(&lines).map(|(h, _)| h)
}

fn log(extra: &str, body: &str) -> Result<session_recorder::SessionLog, OklError> {
    read_str(&format!("{HEAD}{extra}# ---\n{body}"), OnUnknown::Error)
}

const RESPONSE: &str = "# events:\n#   response : id:int operandum:int?\n";

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("session-recorder-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("nested").join("log.txt")
}

fn beyond_i64() -> String {
    (i128::from(i64::MAX) + 1).to_string()
}

#[test]
fn strings_are_escaped_and_restored() {
    for (raw, encoded) in [
        ("plain", "plain"),
        ("a\tb", "a\\tb"),
        ("a\nb", "a\\nb"),
        ("a\rb", "a\\rb"),
        ("a\\b", "a\\\\b"),
        ("-", "\\-"),
        ("--", "--"),
        ("", ""),
        ("日本語\u{2028}行", "日本語\u{2028}行"),
        ("\\t", "\\\\t"),
    ] {
        assert_eq!(encode_str(raw), encoded);
        assert_eq!(decode_str(encoded), raw);
    }
    assert_eq!(decode_str("\\x"), "\\x");
    assert_eq!(decode_str("end\\"), "end\\");
}

#[test]
fn numbers_follow_python_spelling_rules() {
    for (token, value) in [
        ("1.5", 1.5),
        ("1e-3", 1e-3),
        ("-1.0", -1.0),
        (".5", 0.5),
        ("5.", 5.0),
        ("1_0.5", 10.5),
        ("+2", 2.0),
    ] {
        assert_eq!(parse_float(token), Some(value), "{token}");
    }
    for token in ["", "1__0", "_1", "1_", "x", "1e", "0x10", "1_e5"] {
        assert_eq!(parse_float(token), None, "{token}");
    }
    assert_eq!(parse_int("+5"), Some(5));
    assert_eq!(parse_int("-5"), Some(-5));
    assert_eq!(parse_int("1_000"), Some(1000));
    for token in ["", "1.0", "_1", "1_", "-"] {
        assert_eq!(parse_int(token), None, "{token}");
    }
    assert_eq!(parse_int(&i64::MAX.to_string()), Some(i64::MAX));
    assert_eq!(parse_int(&beyond_i64()), None);
}

#[test]
fn events_are_encoded_with_the_canonical_columns() {
    let cases: Vec<(SessionEvent, &str)> = vec![
        (
            ResponseEvent {
                id: 1,
                timestamp: 0.5,
                operandum: None,
                device_time: None,
            }
            .into(),
            "0.5\tresponse\t1\t-\t-",
        ),
        (
            ResponseEvent {
                id: 1,
                timestamp: 0.5,
                operandum: Some(0),
                device_time: Some(1e16),
            }
            .into(),
            "0.5\tresponse\t1\t0\t1e+16",
        ),
        (
            ReinforcerStartEvent {
                id: 2,
                timestamp: 1.0,
                operandum: None,
                potency: 1.0,
            }
            .into(),
            "1.0\treinforcer_start\t2\t1.0\t-",
        ),
        (
            ReinforcerEndEvent {
                id: 2,
                timestamp: 1.5,
                operandum: Some(3),
            }
            .into(),
            "1.5\treinforcer_end\t2\t3",
        ),
        (
            StateChangeEvent {
                from_state: SessionState::Idle,
                to_state: SessionState::Running,
                timestamp: 0.0,
            }
            .into(),
            "0.0\tstate_change\tIDLE\tRUNNING",
        ),
        (
            ComponentChangeEvent {
                from_component: String::new(),
                to_component: "-".into(),
                timestamp: 0.0,
            }
            .into(),
            "0.0\tcomponent_change\t\t\\-",
        ),
    ];
    for (event, line) in cases {
        assert_eq!(encode_event(&event), line);
        let record = parse_event_line(line, &canonical_codebook()).unwrap();
        assert_eq!(session_recorder::materialise(&record).unwrap(), event);
    }
}

#[test]
fn every_session_state_has_its_python_name() {
    for (state, name) in [
        (SessionState::Idle, "IDLE"),
        (SessionState::Running, "RUNNING"),
        (SessionState::Reinforcing, "REINFORCING"),
        (SessionState::Paused, "PAUSED"),
        (SessionState::Finished, "FINISHED"),
    ] {
        assert_eq!(state.name(), name);
        assert_eq!(SessionState::from_name(name), Some(state));
    }
    assert_eq!(SessionState::from_name("idle"), None);
}

#[test]
fn the_full_header_is_written_in_the_canonical_order() {
    let meta = SessionMeta {
        timestamp: 0.0,
        clock_type: "ManualClock".into(),
        session_name: "q\"b\\t\t".into(),
        subject_id: Some("s1".into()),
        replication_index: Some(0),
        experiment_name: Some("e".into()),
        protocol_id: Some("p".into()),
        task_file_hash: Some("h".into()),
        wall_clock_start: Some("2026-01-01T00:00:00Z".into()),
        notes: vec!["n1\nn2".into()],
        metadata: vec![
            ("i".into(), Value::Int(-3)),
            ("f".into(), Value::Float(1e-5)),
            ("b".into(), Value::Bool(true)),
            ("s".into(), Value::Str("x".into())),
        ],
    };
    let mut sink = MemorySink::new(Vec::new());
    sink.write_header(&meta).unwrap();
    let text = sink.into_string();
    assert!(text.starts_with(
        "# OKL v1\n# session_name = \"q\\\"b\\\\t\\t\"\n# clock_type = \"ManualClock\"\n# session_start = 0.0\n\
         # subject_id = \"s1\"\n# replication_index = 0\n# experiment_name = \"e\"\n# protocol_id = \"p\"\n\
         # task_file_hash = \"h\"\n# wall_clock_start = \"2026-01-01T00:00:00Z\"\n# notes:\n#   \"n1\\nn2\"\n\
         # meta:\n#   i = -3\n#   f = 1e-05\n#   b = true\n#   s = \"x\"\n# events:\n#   response          : id:int operandum:int? device_time:float?\n"
    ), "{text}");
    assert!(text.ends_with("#   stimulus_off      : stimulus:str\n# ---\n"));
    assert_eq!(read_str(&text, OnUnknown::Error).unwrap().meta, meta);
}

#[test]
fn malformed_headers_are_rejected() {
    for extra in [
        "# no equals sign\n",
        "session line without hash\n",
        "# events:\n#   response : id\n",
        "# events:\n#   response : id:number\n",
        "# events:\n#   : id:int\n",
        "# meta:\n#   1bad = 2\n",
        "# meta:\n#   k = bare\n",
        "# notes:\n#   bare\n",
        "# replication_index = \"x\"\n",
    ] {
        assert!(matches!(header(extra), Err(OklError::Header(_))), "{extra}");
    }
    let too_large = header(&format!("# replication_index = {}\n", beyond_i64()));
    assert!(matches!(too_large, Err(OklError::Header(_))));
    let eof = read_str(HEAD, OnUnknown::Error);
    assert!(matches!(eof, Err(OklError::Header(m)) if m.contains("EOF")));
    let bom = read_str(&format!("\u{feff}{HEAD}# ---\n"), OnUnknown::Error);
    assert!(matches!(bom, Err(OklError::Header(m)) if m.contains("BOM")));
}

#[test]
fn header_values_are_coerced_as_python_does() {
    let h = header("# subject_id = 5\n# replication_index = 2\n# protocol_id = true\n# experiment_name = 1.0\n").unwrap();
    assert_eq!(h.subject_id.as_deref(), Some("5"));
    assert_eq!(h.replication_index, Some(2));
    assert_eq!(h.protocol_id.as_deref(), Some("True"));
    assert_eq!(h.experiment_name.as_deref(), Some("1.0"));
}

#[test]
fn unknown_top_level_keys_are_kept_and_reported() {
    let h = header("# subject = \"s1\"\n").unwrap();
    assert_eq!(h.unknown_keys, ["subject"]);
    assert_eq!(
        h.metadata,
        [("subject".to_string(), Value::Str("s1".into()))]
    );
}

#[test]
fn a_repeated_meta_key_takes_the_last_value() {
    let h = header("# meta:\n#   a = 1\n#   b = 2\n#   a = 3\n").unwrap();
    assert_eq!(
        h.metadata,
        [
            ("a".to_string(), Value::Int(3)),
            ("b".to_string(), Value::Int(2))
        ]
    );
}

#[test]
fn malformed_body_lines_are_rejected() {
    for body in [
        "0.0\n",
        "x\tresponse\t1\t-\n",
        "0.0\tresponse\t1\n",
        "0.0\tresponse\t1.5\t-\n",
        "0.0\tresponse\t-\t-\n",
    ] {
        assert!(
            matches!(log(RESPONSE, body), Err(OklError::Record(_))),
            "{body}"
        );
    }
    let bool_head = "# events:\n#   flag : on:bool\n";
    assert!(log(bool_head, "0.0\tflag\ttrue\n").is_ok());
    assert!(matches!(
        log(bool_head, "0.0\tflag\t1\n"),
        Err(OklError::Record(_))
    ));
    let state = "# events:\n#   state_change : from:str to:str\n";
    assert!(matches!(
        log(state, "0.0\tstate_change\tIDLE\tNOPE\n"),
        Err(OklError::Record(_))
    ));
    let odd = "# events:\n#   response : number:int\n";
    assert!(matches!(
        log(odd, "0.0\tresponse\t1\n"),
        Err(OklError::Record(_))
    ));
}

#[test]
fn records_reject_values_that_do_not_fit_the_column() {
    let cb = canonical_codebook();
    let id = |v: Option<Value>| vec![("id".to_string(), v)];
    assert!(matches!(
        encode_record(0.0, "reinforcer_end", &id(None), &cb),
        Err(OklError::Record(_))
    ));
    assert!(matches!(
        encode_record(0.0, "reinforcer_end", &id(Some(Value::Bool(true))), &cb),
        Err(OklError::Record(_))
    ));
    assert!(matches!(
        encode_record(0.0, "nope", &[], &cb),
        Err(OklError::UnknownType(_))
    ));
    assert_eq!(
        encode_record(0.0, "reinforcer_end", &id(Some(Value::Int(1))), &cb).unwrap(),
        "0.0\treinforcer_end\t1\t-"
    );
}

#[test]
fn unknown_types_are_skipped_and_reported_unless_error_is_asked() {
    let text = format!("{HEAD}{RESPONSE}# ---\n0.0\tmystery\t1\n0.5\tresponse\t1\t-\n");
    let log = read_str(&text, OnUnknown::Skip).unwrap();
    assert_eq!(log.events.len(), 1);
    assert!(log.unknown_types.contains("mystery"));
    assert!(
        matches!(read_str(&text, OnUnknown::Error), Err(OklError::UnknownType(t)) if t == "mystery")
    );
}

#[test]
fn a_custom_type_stays_in_raw_records() {
    let log = log("# events:\n#   done : ratio:int\n", "1.0\tdone\t2\n").unwrap();
    assert!(log.events.is_empty());
    assert_eq!(log.raw_records[0].get("ratio"), Some(&Value::Int(2)));
}

#[test]
fn crlf_blank_lines_and_a_torn_last_line_are_handled() {
    let text =
        format!("{HEAD}{RESPONSE}# ---\n0.0\tresponse\t1\t-\n\n1.0\tresponse\t2\t-\n2.0\tresp")
            .replace('\n', "\r\n");
    assert_eq!(read_str(&text, OnUnknown::Error).unwrap().events.len(), 2);
}

#[test]
fn only_lf_separates_lines() {
    let log = log(
        "# events:\n#   note : text:str\n",
        "0.0\tnote\ta\u{2028}b\u{85}c\u{b}d\n",
    )
    .unwrap();
    assert_eq!(
        log.raw_records[0].get("text"),
        Some(&Value::Str("a\u{2028}b\u{85}c\u{b}d".into()))
    );
}

#[test]
fn an_empty_or_missing_file_is_an_empty_log() {
    assert_eq!(
        read_str("  \n\n", OnUnknown::Error).unwrap().meta,
        SessionMeta::new("", "", 0.0)
    );
    let missing = temp_path("missing");
    assert!(read_log(&missing, OnUnknown::Error)
        .unwrap()
        .events
        .is_empty());
}

#[test]
fn a_file_torn_inside_a_character_is_read_up_to_the_last_full_line() {
    let path = temp_path("torn");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes =
        format!("{HEAD}# events:\n#   note : text:str\n# ---\n0.0\tnote\ta\n1.0\tnote\t日")
            .into_bytes();
    bytes.pop();
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        read_log(&path, OnUnknown::Error).unwrap().raw_records.len(),
        1
    );
}

#[test]
fn the_file_sink_writes_each_event_through() {
    let path = temp_path("sink");
    let mut sink = OklSink::new(&path).unwrap();
    let event: SessionEvent = ResponseEvent {
        id: 1,
        timestamp: 0.5,
        operandum: None,
        device_time: None,
    }
    .into();
    assert!(sink.emit(&event).is_err(), "the header comes first");
    assert!(!path.exists());
    sink.write_header(&SessionMeta::new("s", "c", 0.0)).unwrap();
    assert!(
        sink.write_header(&SessionMeta::new("s", "c", 0.0)).is_err(),
        "only once"
    );
    sink.emit(&event).unwrap();
    sink.write_phase_enter(
        1.0,
        "p",
        &PhaseContext {
            name: Some("-".into()),
            ..Default::default()
        },
    )
    .unwrap();
    sink.write_stimulus_on(1.0, "tone").unwrap();
    sink.write_stimulus_off(2.0, "tone").unwrap();
    sink.write_phase_exit(2.0, "p").unwrap();
    // Read while the sink is still open.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.ends_with(
        "# ---\n0.5\tresponse\t1\t-\t-\n1.0\tphase_enter\tp\t\\-\t-\t-\t-\n1.0\tstimulus_on\ttone\n2.0\tstimulus_off\ttone\n2.0\tphase_exit\tp\n"
    ), "{text}");
    let mut again = OklSink::new(&path).unwrap();
    assert!(
        again
            .write_header(&SessionMeta::new("s", "c", 0.0))
            .is_err(),
        "the file is not empty"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn an_empty_codebook_is_written_as_the_canonical_one() {
    let mut out = Vec::new();
    session_recorder::format::write_header(
        &mut out,
        &SessionMeta::new("s", "c", 0.0),
        Some(&Codebook::default()),
    )
    .unwrap();
    assert!(String::from_utf8(out)
        .unwrap()
        .contains("#   stimulus_on       : stimulus:str\n"));
}
