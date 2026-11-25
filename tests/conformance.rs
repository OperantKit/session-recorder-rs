//! The golden files of OperantKitLog (`spec/conformance/`).
//!
//! The files are not copied into this crate. Without a checkout the tests
//! print a SKIP line and pass; set `OKL_CONFORMANCE_ROOT` to point at one.

mod common;

use common::{conformance_root, parse_all, txt_files, write_all};
use session_recorder::format::canonical_codebook;
use session_recorder::{read_str, OklError, OnUnknown};
use std::path::PathBuf;

fn files(kind: &str) -> Option<Vec<PathBuf>> {
    let Some(root) = conformance_root() else {
        eprintln!("SKIP: no OperantKitLog/spec/conformance checkout; set OKL_CONFORMANCE_ROOT");
        return None;
    };
    let files = txt_files(&root.join(kind));
    assert!(!files.is_empty(), "no {kind} fixtures");
    Some(files)
}

fn name(path: &std::path::Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

/// The writer does not reproduce these bytes: 03 writes `\-` inside a longer
/// string (the escape is only for the one-character string), and 04 declares
/// no events, which the writer replaces with the canonical declarations.
const NOT_BYTE_IDENTICAL: [&str; 2] = ["03-tsv-escapes.txt", "04-empty-body.txt"];

#[test]
fn valid_files_are_read() {
    let Some(files) = files("valid") else { return };
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        read_str(&text, OnUnknown::Error).unwrap_or_else(|e| panic!("{}: {e}", name(&path)));
    }
}

#[test]
fn valid_files_keep_their_values_when_written_back() {
    let Some(files) = files("valid") else { return };
    for path in files {
        let file = name(&path);
        let text = std::fs::read_to_string(&path).unwrap();
        let first = parse_all(&text).unwrap();
        let second = parse_all(&write_all(&first).unwrap()).unwrap();
        assert_eq!(
            first.header.session_meta(),
            second.header.session_meta(),
            "{file}"
        );
        assert_eq!(first.records, second.records, "{file}");
        if first.header.codebook.is_empty() {
            assert_eq!(second.header.codebook, canonical_codebook(), "{file}");
        } else {
            assert_eq!(first.header.codebook, second.header.codebook, "{file}");
        }
    }
}

#[test]
fn valid_files_are_byte_identical_when_written_back() {
    let Some(files) = files("valid") else { return };
    for path in files {
        let file = name(&path);
        let text = std::fs::read_to_string(&path).unwrap();
        let written = write_all(&parse_all(&text).unwrap()).unwrap();
        if NOT_BYTE_IDENTICAL.contains(&file.as_str()) {
            assert_ne!(written, text, "{file}: remove it from NOT_BYTE_IDENTICAL");
        } else {
            assert_eq!(written, text, "{file}");
        }
    }
}

#[test]
fn invalid_files_are_rejected() {
    let Some(files) = files("invalid") else {
        return;
    };
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            read_str(&text, OnUnknown::Error).is_err(),
            "{}",
            name(&path)
        );
    }
}

/// The invalid files carry a `# violation:` comment, which is itself a
/// malformed header line. Without it they must still fail, and for the
/// reason the comment states.
#[test]
fn invalid_files_are_rejected_for_the_stated_reason() {
    let Some(files) = files("invalid") else {
        return;
    };
    for path in files {
        let file = name(&path);
        let text = std::fs::read_to_string(&path).unwrap();
        let without: String = text
            .lines()
            .filter(|l| !l.starts_with("# violation:"))
            .map(|l| format!("{l}\n"))
            .collect();
        let err = read_str(&without, OnUnknown::Error).expect_err(&file);
        let expected = match file.as_str() {
            "01-missing-required-key.txt" => {
                matches!(&err, OklError::Header(m) if m.contains("session_start"))
            }
            "02-wrong-magic.txt" => matches!(&err, OklError::Header(m) if m.contains("# OKL v1")),
            "03-column-count-mismatch.txt" => {
                matches!(&err, OklError::Record(m) if m.contains("expected 2 payload cols, got 3"))
            }
            "04-required-field-absent.txt" => {
                matches!(&err, OklError::Record(m) if m.contains("required field \"id\""))
            }
            "05-unknown-event-type.txt" => {
                matches!(&err, OklError::UnknownType(t) if t == "mystery")
            }
            other => panic!("add the expected reason for {other}"),
        };
        assert!(expected, "{file}: {err}");
    }
}
