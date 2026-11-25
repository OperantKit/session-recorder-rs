//! Read an OKL v1 log and write it again.
//!
//! ```text
//! cargo run --example rewrite_okl -- in.txt out.txt
//! ```

use session_recorder::format::{
    encode_record, framed_lines, parse_event_line, parse_header, write_header,
};
use session_recorder::OklError;
use std::io::Write;

fn main() -> Result<(), OklError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output] = args.as_slice() else {
        eprintln!("usage: rewrite_okl <in.txt> <out.txt>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(input)?;
    let lines = framed_lines(&text);
    let (header, consumed) = parse_header(&lines)?;
    let mut out = Vec::new();
    write_header(&mut out, &header.session_meta(), Some(&header.codebook))?;
    for line in lines[consumed..].iter().filter(|l| !l.trim().is_empty()) {
        let r = parse_event_line(line, &header.codebook)?;
        let line = encode_record(r.timestamp, &r.type_name, &r.args, &header.codebook)?;
        writeln!(out, "{line}")?;
    }
    std::fs::write(output, out)?;
    Ok(())
}
