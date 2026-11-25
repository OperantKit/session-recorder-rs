//! The wire format: escaping, the header, and single body lines.

use crate::error::OklError;
use crate::events::{
    ComponentChangeEvent, ReinforcerEndEvent, ReinforcerStartEvent, ResponseEvent, SessionEvent,
    SessionMeta, SessionState, StateChangeEvent, Value,
};
use std::io;

/// First line of every OKL v1 file.
pub const MAGIC: &str = "# OKL v1";
/// Line that ends the header.
pub const HEADER_TERMINATOR: &str = "# ---";
/// Column of an optional value that is missing.
pub const ABSENT: &str = "-";

/// Event type of [`ResponseEvent`].
pub const RESPONSE: &str = "response";
/// Event type of [`ReinforcerStartEvent`].
pub const REINFORCER_START: &str = "reinforcer_start";
/// Event type of [`ReinforcerEndEvent`].
pub const REINFORCER_END: &str = "reinforcer_end";
/// Event type of [`StateChangeEvent`].
pub const STATE_CHANGE: &str = "state_change";
/// Event type of [`ComponentChangeEvent`].
pub const COMPONENT_CHANGE: &str = "component_change";
/// Event type of the start of a phase.
pub const PHASE_ENTER: &str = "phase_enter";
/// Event type of the end of a phase.
pub const PHASE_EXIT: &str = "phase_exit";
/// Event type of the onset of a stimulus presentation.
pub const STIMULUS_ON: &str = "stimulus_on";
/// Event type of the offset of a stimulus presentation.
pub const STIMULUS_OFF: &str = "stimulus_off";

/// Type of a body column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    /// Decimal integer.
    Int,
    /// Real number.
    Float,
    /// Escaped string.
    Str,
    /// `true` or `false`.
    Bool,
}

impl FieldType {
    /// The spelling used in a codebook.
    pub fn name(self) -> &'static str {
        match self {
            FieldType::Int => "int",
            FieldType::Float => "float",
            FieldType::Str => "str",
            FieldType::Bool => "bool",
        }
    }
}

/// One column declared in a codebook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// Column name.
    pub name: String,
    /// Column type.
    pub ty: FieldType,
    /// Whether the value may be missing.
    pub optional: bool,
}

/// The event types of a file and their columns, in declaration order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Codebook(Vec<(String, Vec<Field>)>);

impl Codebook {
    /// The columns of `type_name`.
    pub fn get(&self, type_name: &str) -> Option<&[Field]> {
        self.0
            .iter()
            .find(|(name, _)| name == type_name)
            .map(|(_, fields)| fields.as_slice())
    }

    /// Declare `type_name`; a second declaration replaces the first in place.
    pub fn set(&mut self, type_name: impl Into<String>, fields: Vec<Field>) {
        set_key(&mut self.0, type_name.into(), fields);
    }

    /// Whether no type is declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The declarations in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Field])> {
        self.0.iter().map(|(n, f)| (n.as_str(), f.as_slice()))
    }
}

/// Columns of the nine canonical event types; `?` marks an optional column.
const CANONICAL: [(&str, &str); 9] = [
    (RESPONSE, "id:int operandum:int? device_time:float?"),
    (REINFORCER_START, "id:int potency:float operandum:int?"),
    (REINFORCER_END, "id:int operandum:int?"),
    (STATE_CHANGE, "from:str to:str"),
    (COMPONENT_CHANGE, "from:str to:str"),
    (
        PHASE_ENTER,
        "label:str name:str? time:str? location:str? cue:str?",
    ),
    (PHASE_EXIT, "label:str"),
    (STIMULUS_ON, "stimulus:str"),
    (STIMULUS_OFF, "stimulus:str"),
];

/// The codebook of the nine canonical event types.
pub fn canonical_codebook() -> Codebook {
    Codebook(
        CANONICAL
            .iter()
            .map(|(name, columns)| {
                let fields = columns
                    .split(' ')
                    .map(|c| parse_field(c).expect("the canonical columns are well formed"))
                    .collect();
                (name.to_string(), fields)
            })
            .collect(),
    )
}

/// Set `key`, keeping the position of an earlier entry with the same key.
fn set_key<V>(entries: &mut Vec<(String, V)>, key: String, value: V) {
    match entries.iter_mut().find(|(k, _)| *k == key) {
        Some((_, slot)) => *slot = value,
        None => entries.push((key, value)),
    }
}

// ---------------------------------------------------------------------------
// Scalars
// ---------------------------------------------------------------------------

/// Spell a float as Python's `repr` does.
///
/// Both languages print the fewest digits that read back as the same float.
/// They differ in the exponent (`1e+16` / `1e-05` against Rust's `1e16` /
/// `1e-5`), in `nan`, and where the float lies halfway between two spellings
/// of that length: Python takes the one with the even last digit.
pub fn fmt_float(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() || value == 0.0 {
        return format!("{value:?}");
    }
    let shortest = format!("{:e}", value.abs());
    let digits = shortest.bytes().take_while(|b| *b != b'e');
    let after_point = digits.filter(u8::is_ascii_digit).count() - 1;
    // Rounding the exact value to that many digits breaks ties to even. Next
    // to a power of two the result can be another float; then the shortest
    // spelling stands.
    let rounded = format!("{:.after_point$e}", value.abs());
    let text = if rounded.parse() == Ok(value.abs()) {
        rounded
    } else {
        shortest
    };
    let (mantissa, exponent) = text.split_once('e').expect("`{:e}` writes an exponent");
    let exponent: i32 = exponent.parse().expect("the exponent is an integer");
    let sign = if value < 0.0 { "-" } else { "" };
    if !(-4..16).contains(&exponent) {
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        return format!("{sign}{mantissa}e{exponent_sign}{:02}", exponent.abs());
    }
    let digits = mantissa.replace('.', "");
    match usize::try_from(exponent) {
        Ok(exponent) => {
            let digits = format!("{digits:0<width$}", width = exponent + 2);
            let (whole, fraction) = digits.split_at(exponent + 1);
            format!("{sign}{whole}.{fraction}")
        }
        Err(_) => {
            let zeros = "0".repeat(exponent.unsigned_abs() as usize - 1);
            format!("{sign}0.{zeros}{digits}")
        }
    }
}

/// Remove the underscores Python allows between two digits.
fn strip_underscores(token: &str) -> Option<String> {
    let bytes = token.as_bytes();
    let digit = |i: Option<usize>| i.and_then(|i| bytes.get(i)).is_some_and(u8::is_ascii_digit);
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'_' && !(digit(i.checked_sub(1)) && digit(Some(i + 1))) {
            return None;
        }
    }
    Some(token.replace('_', ""))
}

/// Read any ASCII spelling that Python's `float()` accepts.
pub fn parse_float(token: &str) -> Option<f64> {
    strip_underscores(token.trim())?.parse().ok()
}

/// Read any ASCII spelling that Python's `int()` accepts and an `i64` holds.
pub fn parse_int(token: &str) -> Option<i64> {
    int_digits(token)?.parse().ok()
}

/// `token` without underscores if it is spelled as an integer of any size.
fn int_digits(token: &str) -> Option<String> {
    let text = strip_underscores(token.trim())?;
    let digits = text.strip_prefix(['+', '-']).unwrap_or(&text);
    let is_int = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    is_int.then_some(text)
}

/// `str(value)` of Python.
fn py_str(value: &Value) -> String {
    match value {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => fmt_float(*f),
        Value::Str(s) => s.clone(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
    }
}

/// `float(value)` of Python.
fn py_float(value: &Value) -> Option<f64> {
    match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Str(s) => parse_float(s),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
    }
}

/// `int(value)` of Python, limited to the range of an `i64`.
fn py_int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(i) => Some(*i),
        // -2^63 is exact as a float, so the half-open range is the i64 range.
        Value::Float(f) => (i64::MIN as f64..-(i64::MIN as f64))
            .contains(f)
            .then_some(*f as i64),
        Value::Str(s) => parse_int(s),
        Value::Bool(b) => Some(i64::from(*b)),
    }
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

/// Escape a string for a body column.
pub fn encode_str(value: &str) -> String {
    if value == ABSENT {
        return "\\-".to_string();
    }
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// Inverse of [`encode_str`]. An unknown escape is kept as it is.
pub fn decode_str(token: &str) -> String {
    let mut out = String::with_capacity(token.len());
    let mut chars = token.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('-') => out.push('-'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A TOML basic string.
fn toml_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn toml_format(value: &Value) -> String {
    match value {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => fmt_float(*f),
        Value::Str(s) => toml_quote(s),
        Value::Bool(b) => b.to_string(),
    }
}

fn parse_toml_value(token: &str) -> Result<Value, OklError> {
    if token == "true" || token == "false" {
        return Ok(Value::Bool(token == "true"));
    }
    if token.starts_with('"') && token.ends_with('"') {
        // A lone quote reads as the empty string, as in Python.
        let body = token.get(1..token.len() - 1).unwrap_or_default();
        let mut out = String::with_capacity(body.len());
        let mut chars = body.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        }
        return Ok(Value::Str(out));
    }
    if int_digits(token).is_some() {
        return parse_int(token).map(Value::Int).ok_or_else(|| {
            OklError::Header(format!("integer out of the 64-bit range: {token:?}"))
        });
    }
    parse_float(token)
        .map(Value::Float)
        .ok_or_else(|| OklError::Header(format!("unparseable value: {token:?}")))
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

/// Write the magic line, the header and the terminator.
///
/// `codebook` defaults to [`canonical_codebook`]; an empty one is replaced by
/// it, as the Python writer does.
pub fn write_header<W: io::Write>(
    out: &mut W,
    meta: &SessionMeta,
    codebook: Option<&Codebook>,
) -> Result<(), OklError> {
    let mut text = format!(
        "{MAGIC}\n# session_name = {}\n# clock_type = {}\n# session_start = {}\n",
        toml_quote(&meta.session_name),
        toml_quote(&meta.clock_type),
        fmt_float(meta.timestamp),
    );
    let mut line = |key: &str, value: Option<String>| {
        if let Some(value) = value {
            text.push_str(&format!("# {key} = {value}\n"));
        }
    };
    let quoted = |value: &Option<String>| value.as_deref().map(toml_quote);
    line("subject_id", quoted(&meta.subject_id));
    line(
        "replication_index",
        meta.replication_index.map(|i| i.to_string()),
    );
    line("experiment_name", quoted(&meta.experiment_name));
    line("protocol_id", quoted(&meta.protocol_id));
    line("task_file_hash", quoted(&meta.task_file_hash));
    line("wall_clock_start", quoted(&meta.wall_clock_start));
    if !meta.notes.is_empty() {
        text.push_str("# notes:\n");
        for note in &meta.notes {
            text.push_str(&format!("#   {}\n", toml_quote(note)));
        }
    }
    if !meta.metadata.is_empty() {
        text.push_str("# meta:\n");
        for (key, value) in &meta.metadata {
            text.push_str(&format!("#   {key} = {}\n", toml_format(value)));
        }
    }
    text.push_str("# events:\n");
    let canonical;
    let codebook = match codebook {
        Some(codebook) if !codebook.is_empty() => codebook,
        _ => {
            canonical = canonical_codebook();
            &canonical
        }
    };
    for (type_name, fields) in codebook.iter() {
        let columns: Vec<String> = fields
            .iter()
            .map(|f| {
                let suffix = if f.optional { "?" } else { "" };
                format!("{}:{}{suffix}", f.name, f.ty.name())
            })
            .collect();
        text.push_str(&format!("#   {type_name:<18}: {}\n", columns.join(" ")));
    }
    text.push_str(HEADER_TERMINATOR);
    text.push('\n');
    out.write_all(text.as_bytes())?;
    Ok(())
}

/// The result of [`parse_header`].
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedHeader {
    /// `session_name`
    pub session_name: String,
    /// `clock_type`
    pub clock_type: String,
    /// `session_start`
    pub session_start: f64,
    /// `subject_id`
    pub subject_id: Option<String>,
    /// `replication_index`
    pub replication_index: Option<i64>,
    /// `experiment_name`
    pub experiment_name: Option<String>,
    /// `protocol_id`
    pub protocol_id: Option<String>,
    /// `task_file_hash`
    pub task_file_hash: Option<String>,
    /// `wall_clock_start`, as written
    pub wall_clock_start: Option<String>,
    /// The `notes:` section.
    pub notes: Vec<String>,
    /// The `meta:` section, plus any unknown top-level key.
    pub metadata: Vec<(String, Value)>,
    /// The `events:` section.
    pub codebook: Codebook,
    /// Top-level keys the format does not define (likely typos); their values
    /// are in `metadata`.
    pub unknown_keys: Vec<String>,
}

impl ParsedHeader {
    /// The header as [`SessionMeta`].
    pub fn session_meta(&self) -> SessionMeta {
        SessionMeta {
            timestamp: self.session_start,
            clock_type: self.clock_type.clone(),
            session_name: self.session_name.clone(),
            subject_id: self.subject_id.clone(),
            replication_index: self.replication_index,
            experiment_name: self.experiment_name.clone(),
            protocol_id: self.protocol_id.clone(),
            task_file_hash: self.task_file_hash.clone(),
            wall_clock_start: self.wall_clock_start.clone(),
            notes: self.notes.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

/// Split a file into its complete lines, without the line ends.
///
/// Lines end with LF or CRLF and with nothing else. A last line that has no
/// line end is the trace of an interrupted write and is dropped.
pub fn framed_lines(text: &str) -> Vec<&str> {
    match text.rfind('\n') {
        None => Vec::new(),
        Some(last) => text[..last]
            .split('\n')
            .map(|line| line.trim_end_matches('\r'))
            .collect(),
    }
}

/// Whether `name` starts with an ASCII letter or `_` and goes on with letters,
/// digits, `_` or a character of `extra`.
fn is_name(name: &str, extra: &[char]) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || extra.contains(&c))
}

fn parse_field(token: &str) -> Option<Field> {
    let (name, ty) = token.split_once(':')?;
    let (ty, optional) = match ty.strip_suffix('?') {
        Some(ty) => (ty, true),
        None => (ty, false),
    };
    let ty = match ty {
        "int" => FieldType::Int,
        "float" => FieldType::Float,
        "str" => FieldType::Str,
        "bool" => FieldType::Bool,
        _ => return None,
    };
    is_name(name, &[]).then(|| Field {
        name: name.to_string(),
        ty,
        optional,
    })
}

/// Split a `key = value` line.
fn key_value(body: &str) -> Option<(&str, &str)> {
    let (key, value) = body.split_once('=')?;
    let (key, value) = (key.trim(), value.trim());
    (is_name(key, &['.', '-']) && !value.is_empty()).then_some((key, value))
}

#[derive(Clone, Copy)]
enum Section {
    Top,
    Meta,
    Notes,
    Events,
}

/// Parse the header out of a file's lines, as [`framed_lines`] gives them.
/// Returns it with the number of lines it takes up.
pub fn parse_header<S: AsRef<str>>(lines: &[S]) -> Result<(ParsedHeader, usize), OklError> {
    let fail = |message: String| Err(OklError::Header(message));
    let Some(first) = lines.first().map(AsRef::as_ref) else {
        return fail("unexpected EOF".to_string());
    };
    if first.starts_with('\u{feff}') {
        return fail("the file starts with a UTF-8 BOM; save it without one".to_string());
    }
    if first != MAGIC {
        return fail(format!("expected {MAGIC:?} as first line, got {first:?}"));
    }

    let mut session_name = None;
    let mut clock_type = None;
    let mut session_start = None;
    // The required fields of `header` are filled in at the end.
    let mut header = ParsedHeader {
        session_name: String::new(),
        clock_type: String::new(),
        session_start: 0.0,
        subject_id: None,
        replication_index: None,
        experiment_name: None,
        protocol_id: None,
        task_file_hash: None,
        wall_clock_start: None,
        notes: Vec::new(),
        metadata: Vec::new(),
        codebook: Codebook::default(),
        unknown_keys: Vec::new(),
    };
    let mut section = Section::Top;
    let mut consumed = None;
    for (i, raw) in lines.iter().map(AsRef::as_ref).enumerate().skip(1) {
        if raw == HEADER_TERMINATOR {
            consumed = Some(i + 1);
            break;
        }
        let Some(body) = raw.strip_prefix('#') else {
            return fail(format!("not a header line: {raw:?}"));
        };
        let body = body.trim_start();
        if body.is_empty() {
            continue;
        }
        if body.starts_with("meta:") {
            section = Section::Meta;
            continue;
        }
        if body.starts_with("notes:") {
            section = Section::Notes;
            continue;
        }
        if body.starts_with("events:") {
            section = Section::Events;
            continue;
        }
        let malformed = || OklError::Header(format!("malformed line: {raw:?}"));
        match section {
            Section::Meta => {
                let (key, value) = key_value(body).ok_or_else(malformed)?;
                set_key(
                    &mut header.metadata,
                    key.to_string(),
                    parse_toml_value(value)?,
                );
            }
            Section::Notes => match parse_toml_value(body)? {
                Value::Str(note) => header.notes.push(note),
                _ => return Err(malformed()),
            },
            Section::Events => {
                let (type_name, rest) = body.split_once(':').unwrap_or((body, ""));
                let type_name = type_name.trim();
                if type_name.is_empty() {
                    return Err(malformed());
                }
                let fields = rest
                    .split_whitespace()
                    .map(|token| {
                        parse_field(token).ok_or_else(|| {
                            OklError::Header(format!("malformed codebook field: {token:?}"))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                header.codebook.set(type_name, fields);
            }
            Section::Top => {
                let (key, token) = key_value(body).ok_or_else(malformed)?;
                let value = parse_toml_value(token)?;
                let wrong = || OklError::Header(format!("bad value for {key}: {token}"));
                match key {
                    "session_name" => session_name = Some(py_str(&value)),
                    "clock_type" => clock_type = Some(py_str(&value)),
                    "session_start" => {
                        session_start = Some(py_float(&value).ok_or_else(wrong)?);
                    }
                    "subject_id" => header.subject_id = Some(py_str(&value)),
                    "replication_index" => {
                        header.replication_index = Some(py_int(&value).ok_or_else(wrong)?);
                    }
                    "experiment_name" => header.experiment_name = Some(py_str(&value)),
                    "protocol_id" => header.protocol_id = Some(py_str(&value)),
                    "task_file_hash" => header.task_file_hash = Some(py_str(&value)),
                    "wall_clock_start" => header.wall_clock_start = Some(py_str(&value)),
                    _ => {
                        if !header.unknown_keys.iter().any(|k| k == key) {
                            header.unknown_keys.push(key.to_string());
                        }
                        set_key(&mut header.metadata, key.to_string(), value);
                    }
                }
            }
        }
    }
    let Some(consumed) = consumed else {
        return fail("unexpected EOF".to_string());
    };
    match (session_name, clock_type, session_start) {
        (Some(session_name), Some(clock_type), Some(session_start)) => {
            header.session_name = session_name;
            header.clock_type = clock_type;
            header.session_start = session_start;
            Ok((header, consumed))
        }
        (name, clock, start) => {
            let missing: Vec<&str> = [
                ("session_name", name.is_none()),
                ("clock_type", clock.is_none()),
                ("session_start", start.is_none()),
            ]
            .iter()
            .filter(|(_, missing)| *missing)
            .map(|(key, _)| *key)
            .collect();
            fail(format!("missing required keys: {}", missing.join(", ")))
        }
    }
}

// ---------------------------------------------------------------------------
// Body lines: encode
// ---------------------------------------------------------------------------

fn encode_value(value: Option<&Value>, field: &Field) -> Result<String, OklError> {
    let Some(value) = value else {
        return if field.optional {
            Ok(ABSENT.to_string())
        } else {
            Err(OklError::Record(format!(
                "required field {:?} is missing",
                field.name
            )))
        };
    };
    match (field.ty, value) {
        (FieldType::Int, Value::Int(i)) => Ok(i.to_string()),
        (FieldType::Float, Value::Float(f)) => Ok(fmt_float(*f)),
        (FieldType::Float, Value::Int(i)) => Ok(fmt_float(*i as f64)),
        (FieldType::Bool, Value::Bool(b)) => Ok(b.to_string()),
        (FieldType::Str, Value::Str(s)) => Ok(encode_str(s)),
        (ty, value) => Err(OklError::Record(format!(
            "field {:?} is declared {} but got {value:?}",
            field.name,
            ty.name()
        ))),
    }
}

/// Encode one body line (no line end) of any type `codebook` declares. A
/// column that `args` does not name is written as missing.
pub fn encode_record(
    timestamp: f64,
    type_name: &str,
    args: &[(String, Option<Value>)],
    codebook: &Codebook,
) -> Result<String, OklError> {
    let fields = codebook
        .get(type_name)
        .ok_or_else(|| OklError::UnknownType(type_name.to_string()))?;
    let mut line = format!("{}\t{type_name}", fmt_float(timestamp));
    for field in fields {
        let value = args
            .iter()
            .find(|(name, _)| *name == field.name)
            .and_then(|(_, value)| value.as_ref());
        line.push('\t');
        line.push_str(&encode_value(value, field)?);
    }
    Ok(line)
}

fn opt_int(value: Option<i64>) -> String {
    value.map_or_else(|| ABSENT.to_string(), |i| i.to_string())
}

fn opt_str(value: &Option<String>) -> String {
    value
        .as_deref()
        .map_or_else(|| ABSENT.to_string(), encode_str)
}

/// Encode an event as one body line (no line end), with the canonical columns.
pub fn encode_event(event: &SessionEvent) -> String {
    match event {
        SessionEvent::Response(e) => format!(
            "{}\t{RESPONSE}\t{}\t{}\t{}",
            fmt_float(e.timestamp),
            e.id,
            opt_int(e.operandum),
            e.device_time.map_or_else(|| ABSENT.to_string(), fmt_float),
        ),
        SessionEvent::ReinforcerStart(e) => format!(
            "{}\t{REINFORCER_START}\t{}\t{}\t{}",
            fmt_float(e.timestamp),
            e.id,
            fmt_float(e.potency),
            opt_int(e.operandum),
        ),
        SessionEvent::ReinforcerEnd(e) => format!(
            "{}\t{REINFORCER_END}\t{}\t{}",
            fmt_float(e.timestamp),
            e.id,
            opt_int(e.operandum),
        ),
        SessionEvent::StateChange(e) => format!(
            "{}\t{STATE_CHANGE}\t{}\t{}",
            fmt_float(e.timestamp),
            e.from_state.name(),
            e.to_state.name(),
        ),
        SessionEvent::ComponentChange(e) => format!(
            "{}\t{COMPONENT_CHANGE}\t{}\t{}",
            fmt_float(e.timestamp),
            encode_str(&e.from_component),
            encode_str(&e.to_component),
        ),
    }
}

/// The context of a phase, as a `phase_enter` line carries it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhaseContext {
    /// Context name.
    pub name: Option<String>,
    /// Temporal coordinate.
    pub time: Option<String>,
    /// Spatial coordinate.
    pub location: Option<String>,
    /// Stimulus coordinate.
    pub cue: Option<String>,
}

/// Encode the start of a phase.
pub fn encode_phase_enter(timestamp: f64, label: &str, context: &PhaseContext) -> String {
    format!(
        "{}\t{PHASE_ENTER}\t{}\t{}\t{}\t{}\t{}",
        fmt_float(timestamp),
        encode_str(label),
        opt_str(&context.name),
        opt_str(&context.time),
        opt_str(&context.location),
        opt_str(&context.cue),
    )
}

/// Encode the end of a phase.
pub fn encode_phase_exit(timestamp: f64, label: &str) -> String {
    format!(
        "{}\t{PHASE_EXIT}\t{}",
        fmt_float(timestamp),
        encode_str(label)
    )
}

/// Encode the onset of a stimulus presentation.
pub fn encode_stimulus_on(timestamp: f64, stimulus: &str) -> String {
    format!(
        "{}\t{STIMULUS_ON}\t{}",
        fmt_float(timestamp),
        encode_str(stimulus)
    )
}

/// Encode the offset of a stimulus presentation.
pub fn encode_stimulus_off(timestamp: f64, stimulus: &str) -> String {
    format!(
        "{}\t{STIMULUS_OFF}\t{}",
        fmt_float(timestamp),
        encode_str(stimulus)
    )
}

// ---------------------------------------------------------------------------
// Body lines: decode
// ---------------------------------------------------------------------------

/// One body line, its columns named by the file's codebook.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedRecord {
    /// Session time.
    pub timestamp: f64,
    /// Event type.
    pub type_name: String,
    /// Columns in order; `None` for a missing optional value.
    pub args: Vec<(String, Option<Value>)>,
}

impl ParsedRecord {
    /// The value of a column; `None` if it is missing or not declared.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.args
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, value)| value.as_ref())
    }
}

fn decode_value(token: &str, field: &Field) -> Result<Option<Value>, OklError> {
    if token == ABSENT {
        return if field.optional {
            Ok(None)
        } else {
            Err(OklError::Record(format!(
                "required field {:?} got the absent token",
                field.name
            )))
        };
    }
    let value = match field.ty {
        FieldType::Int => parse_int(token).map(Value::Int),
        FieldType::Float => parse_float(token).map(Value::Float),
        FieldType::Str => Some(Value::Str(decode_str(token))),
        FieldType::Bool => match token {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
    };
    value.map(Some).ok_or_else(|| {
        OklError::Record(format!(
            "field {:?} is declared {} but got {token:?}",
            field.name,
            field.ty.name()
        ))
    })
}

/// Parse one body line according to `codebook`.
pub fn parse_event_line(line: &str, codebook: &Codebook) -> Result<ParsedRecord, OklError> {
    let line = line.trim_end_matches(['\r', '\n']);
    let cols: Vec<&str> = line.split('\t').collect();
    let [timestamp, type_name, payload @ ..] = cols.as_slice() else {
        return Err(OklError::Record(format!(
            "a line needs at least 2 columns: {line:?}"
        )));
    };
    let timestamp = parse_float(timestamp)
        .ok_or_else(|| OklError::Record(format!("bad timestamp: {timestamp:?}")))?;
    let fields = codebook
        .get(type_name)
        .ok_or_else(|| OklError::UnknownType(type_name.to_string()))?;
    if payload.len() != fields.len() {
        return Err(OklError::Record(format!(
            "event {type_name:?}: expected {} payload cols, got {}",
            fields.len(),
            payload.len()
        )));
    }
    let args = fields
        .iter()
        .zip(payload)
        .map(|(field, token)| Ok((field.name.clone(), decode_value(token, field)?)))
        .collect::<Result<Vec<_>, OklError>>()?;
    Ok(ParsedRecord {
        timestamp,
        type_name: type_name.to_string(),
        args,
    })
}

/// Build the event a record stands for.
///
/// Only the five types with a [`SessionEvent`] have one; a phase marker, a
/// stimulus presentation or an extension type is an error.
pub fn materialise(record: &ParsedRecord) -> Result<SessionEvent, OklError> {
    let bad = |name: &str| {
        OklError::Record(format!(
            "event {:?}: field {name:?} is missing or has the wrong type",
            record.type_name
        ))
    };
    let int = |name: &str| match record.get(name) {
        Some(Value::Int(i)) => Ok(*i),
        _ => Err(bad(name)),
    };
    let float = |name: &str| match record.get(name) {
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Int(i)) => Ok(*i as f64),
        _ => Err(bad(name)),
    };
    let text = |name: &str| match record.get(name) {
        Some(Value::Str(s)) => Ok(s.clone()),
        _ => Err(bad(name)),
    };
    let opt_int = |name: &str| record.get(name).map(|_| int(name)).transpose();
    let opt_float = |name: &str| record.get(name).map(|_| float(name)).transpose();
    let state = |name: &str| SessionState::from_name(&text(name)?).ok_or_else(|| bad(name));
    let timestamp = record.timestamp;
    Ok(match record.type_name.as_str() {
        RESPONSE => ResponseEvent {
            id: int("id")?,
            timestamp,
            operandum: opt_int("operandum")?,
            device_time: opt_float("device_time")?,
        }
        .into(),
        REINFORCER_START => ReinforcerStartEvent {
            id: int("id")?,
            timestamp,
            operandum: opt_int("operandum")?,
            potency: float("potency")?,
        }
        .into(),
        REINFORCER_END => ReinforcerEndEvent {
            id: int("id")?,
            timestamp,
            operandum: opt_int("operandum")?,
        }
        .into(),
        STATE_CHANGE => StateChangeEvent {
            from_state: state("from")?,
            to_state: state("to")?,
            timestamp,
        }
        .into(),
        COMPONENT_CHANGE => ComponentChangeEvent {
            from_component: text("from")?,
            to_component: text("to")?,
            timestamp,
        }
        .into(),
        other => {
            return Err(OklError::Record(format!(
                "event type {other:?} has no session event"
            )))
        }
    })
}
