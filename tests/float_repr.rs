//! `fmt_float` against Python's `repr` (`tests/fixtures/float_repr.tsv`,
//! written by `scripts/gen_float_repr.py`).

mod common;

use session_recorder::format::{fmt_float, parse_float};

#[test]
fn fmt_float_spells_floats_as_python_does() {
    let text = std::fs::read_to_string(common::fixture("float_repr.tsv")).unwrap();
    let mut with_exponent = 0;
    for line in text.lines() {
        let (bits, repr) = line.split_once('\t').unwrap();
        let value = f64::from_bits(u64::from_str_radix(bits, 16).unwrap());
        assert_eq!(fmt_float(value), repr, "{bits}");
        with_exponent += usize::from(repr.contains('e'));
        let back = parse_float(repr).unwrap();
        assert!(back.to_bits() == value.to_bits() || (back.is_nan() && value.is_nan()));
    }
    assert!(with_exponent > 100, "{with_exponent}");
}
