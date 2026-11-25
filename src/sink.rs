use crate::error::OklError;
use crate::events::{SessionEvent, SessionMeta};
use crate::format::{self, PhaseContext};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Accepts session events as they are emitted.
pub trait EventSink {
    /// Take one event.
    fn emit(&mut self, event: &SessionEvent) -> Result<(), OklError>;
}

/// Writes an OKL v1 log to any [`Write`]: the header once, then one line per
/// record. Each line reaches the writer in a single `write_all`.
#[derive(Debug)]
pub struct OklWriter<W: Write> {
    out: W,
    header_written: bool,
}

/// An [`OklWriter`] that keeps the log in memory.
pub type MemorySink = OklWriter<Vec<u8>>;

impl<W: Write> OklWriter<W> {
    /// Wrap a writer. Nothing is written until [`OklWriter::write_header`].
    pub fn new(out: W) -> Self {
        OklWriter {
            out,
            header_written: false,
        }
    }

    /// Write the header. It comes first and only once.
    pub fn write_header(&mut self, meta: &SessionMeta) -> Result<(), OklError> {
        if self.header_written {
            return Err(OklError::Header("already written".to_string()));
        }
        format::write_header(&mut self.out, meta, None)?;
        self.header_written = true;
        Ok(())
    }

    /// Write the start of a phase.
    pub fn write_phase_enter(
        &mut self,
        timestamp: f64,
        label: &str,
        context: &PhaseContext,
    ) -> Result<(), OklError> {
        self.line(format::encode_phase_enter(timestamp, label, context))
    }

    /// Write the end of a phase.
    pub fn write_phase_exit(&mut self, timestamp: f64, label: &str) -> Result<(), OklError> {
        self.line(format::encode_phase_exit(timestamp, label))
    }

    /// Write the onset of a stimulus presentation.
    pub fn write_stimulus_on(&mut self, timestamp: f64, stimulus: &str) -> Result<(), OklError> {
        self.line(format::encode_stimulus_on(timestamp, stimulus))
    }

    /// Write the offset of a stimulus presentation.
    pub fn write_stimulus_off(&mut self, timestamp: f64, stimulus: &str) -> Result<(), OklError> {
        self.line(format::encode_stimulus_off(timestamp, stimulus))
    }

    /// Give the wrapped writer back.
    pub fn into_inner(self) -> W {
        self.out
    }

    fn line(&mut self, mut line: String) -> Result<(), OklError> {
        if !self.header_written {
            return Err(OklError::Header("not written yet".to_string()));
        }
        line.push('\n');
        self.out.write_all(line.as_bytes())?;
        Ok(())
    }
}

impl MemorySink {
    /// The log as text.
    pub fn into_string(self) -> String {
        String::from_utf8(self.into_inner()).expect("only UTF-8 is written")
    }
}

impl<W: Write> EventSink for OklWriter<W> {
    fn emit(&mut self, event: &SessionEvent) -> Result<(), OklError> {
        self.line(format::encode_event(event))
    }
}

/// Writes an OKL v1 log to a file, unbuffered: a line that was written stays
/// readable if the program stops.
#[derive(Debug)]
pub struct OklSink {
    path: PathBuf,
    writer: Option<OklWriter<File>>,
}

impl OklSink {
    /// Prepare a log at `path`, making its parent directories. The file itself
    /// is created by [`OklSink::write_header`].
    pub fn new(path: impl AsRef<Path>) -> Result<Self, OklError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        Ok(OklSink { path, writer: None })
    }

    /// Where the log is written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write the header. It comes first, only once, and only to a file that
    /// is new or empty.
    pub fn write_header(&mut self, meta: &SessionMeta) -> Result<(), OklError> {
        if self.writer.is_some() {
            return Err(OklError::Header("already written".to_string()));
        }
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        if file.metadata()?.len() > 0 {
            return Err(OklError::Header(format!(
                "{} is not empty; the header must be at byte 0",
                self.path.display()
            )));
        }
        let mut writer = OklWriter::new(file);
        writer.write_header(meta)?;
        self.writer = Some(writer);
        Ok(())
    }

    /// Write the start of a phase.
    pub fn write_phase_enter(
        &mut self,
        timestamp: f64,
        label: &str,
        context: &PhaseContext,
    ) -> Result<(), OklError> {
        self.writer()?.write_phase_enter(timestamp, label, context)
    }

    /// Write the end of a phase.
    pub fn write_phase_exit(&mut self, timestamp: f64, label: &str) -> Result<(), OklError> {
        self.writer()?.write_phase_exit(timestamp, label)
    }

    /// Write the onset of a stimulus presentation.
    pub fn write_stimulus_on(&mut self, timestamp: f64, stimulus: &str) -> Result<(), OklError> {
        self.writer()?.write_stimulus_on(timestamp, stimulus)
    }

    /// Write the offset of a stimulus presentation.
    pub fn write_stimulus_off(&mut self, timestamp: f64, stimulus: &str) -> Result<(), OklError> {
        self.writer()?.write_stimulus_off(timestamp, stimulus)
    }

    fn writer(&mut self) -> Result<&mut OklWriter<File>, OklError> {
        self.writer
            .as_mut()
            .ok_or_else(|| OklError::Header("not written yet".to_string()))
    }
}

impl EventSink for OklSink {
    fn emit(&mut self, event: &SessionEvent) -> Result<(), OklError> {
        self.writer()?.emit(event)
    }
}
