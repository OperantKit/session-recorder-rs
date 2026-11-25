# session-recorder-rs

:jp: [日本語版 README](README.ja.md)

Reads and writes OKL v1 (OperantKitLog v1) session logs in Rust. The files are
the same as those of the Python
[session-recorder](https://github.com/OperantKit/session-recorder); the format
is specified by [OperantKitLog](https://github.com/OperantKit/OperantKitLog).

No external dependencies. Running a session is not this crate's job, and it
depends on no session engine.

## Use

The crate is named `session-recorder`:

```toml
[dependencies]
session-recorder = { git = "https://github.com/OperantKit/session-recorder-rs" }
```

Write a log, then read it back:

```rust,no_run
use session_recorder::{
    read_log, EventSink, OklError, OklSink, OnUnknown, ResponseEvent, SessionMeta,
};

fn main() -> Result<(), OklError> {
    let mut sink = OklSink::new("demo.txt")?;
    sink.write_header(&SessionMeta::new("demo", "ManualClock", 0.0))?;
    sink.emit(
        &ResponseEvent {
            id: 1,
            timestamp: 0.5,
            operandum: None,
            device_time: None,
        }
        .into(),
    )?;
    sink.write_stimulus_on(1.0, "tone")?;
    sink.write_stimulus_off(2.0, "tone")?;

    let log = read_log("demo.txt", OnUnknown::Error)?;
    assert_eq!(log.events.len(), 1);
    assert_eq!(log.raw_records.len(), 3);
    Ok(())
}
```

`OklSink` writes to a file, `MemorySink` keeps the log in memory, and
`OklWriter` takes any `std::io::Write`.

## What it shares with the Python package

- The header, the canonical codebook, the escapes and the columns of each
  event type.
- The spelling of floats: `format::fmt_float` gives the same characters as
  Python's `repr(float)`.

Checked by the tests:

- every valid file of OperantKitLog is read and keeps its values when written
  back, and every invalid file is rejected for the reason it states;
- of the valid files, all are reproduced byte for byte except
  `03-tsv-escapes.txt` and `04-empty-body.txt`, which the Python writer does
  not reproduce either;
- logs written by Python (`tests/fixtures/okl/`) are reproduced byte for byte.

`scripts/check_python_roundtrip.py` checks the other direction: Python reads
what this crate writes.

## Where it differs

| Case | Python | This crate |
|---|---|---|
| A type the codebook does not declare | `"warn"`, `"skip"` or `"error"` | `OnUnknown::Skip` or `OnUnknown::Error`; nothing is printed, skipped types are in `SessionLog::unknown_types` |
| An unknown top-level header key | warns, keeps it in `metadata` | keeps it in `metadata` and lists it in `unknown_keys` |
| `wall_clock_start` | parsed as a date and time | kept as the string in the file, not checked |
| Line ends | every Unicode line end separates lines | only LF and CRLF, as the specification says |
| A last line cut inside a character | decoding error | dropped like any incomplete last line |
| Integers | any size | 64-bit; a larger one is an error |

Reading MED-PC files is not included.

## Tests

```bash
cargo test
```

The conformance tests read the files of OperantKitLog from a checkout beside
one of the parent directories, or from `OKL_CONFORMANCE_ROOT` (the directory
holding `valid/` and `invalid/`). Without either they print `SKIP` and pass;
`cargo test -- --nocapture` shows it.

## License

MIT. See [LICENSE](LICENSE).
