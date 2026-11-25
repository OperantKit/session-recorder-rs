# session-recorder-rs

:gb: [English README](README.md)

OKL v1 (OperantKitLog v1) のセッションの記録を Rust で読み書きする。ファイルは
Python の [session-recorder](https://github.com/OperantKit/session-recorder) と
同じ物で、形式の仕様は
[OperantKitLog](https://github.com/OperantKit/OperantKitLog) にある。

外部の crate には依存しない。セッションを進めるのはこの crate の仕事ではなく、
セッションのエンジンにも依存しない。

## 使い方

crate の名前は `session-recorder`:

```toml
[dependencies]
session-recorder = { git = "https://github.com/OperantKit/session-recorder-rs" }
```

記録を書き、読み戻す:

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

`OklSink` はファイルに書き、`MemorySink` は記録をメモリに持ち、`OklWriter` は任意の
`std::io::Write` に書く。

## Python のパッケージと同じ点

- ヘッダ、正式な codebook、エスケープ、出来事の種類ごとの列。
- 小数の書き方: `format::fmt_float` は Python の `repr(float)` と同じ文字列を返す。

テストで確かめていること:

- OperantKitLog の valid のファイルは全部読めて、書き戻しても値が変わらない。
  invalid のファイルは全部、ファイルに書かれた理由で拒否する。
- valid のファイルは、`03-tsv-escapes.txt` と `04-empty-body.txt` を除いて、書き戻すと
  バイト単位で一致する。この 2 つは Python の writer でも一致しない。
- Python が書いた記録 (`tests/fixtures/okl/`) は、書き直すとバイト単位で一致する。

逆の向き (この crate が書いた物を Python が読む) は
`scripts/check_python_roundtrip.py` で確かめる。

## 違う点

| 場面 | Python | この crate |
|---|---|---|
| codebook に無い種類 | `"warn"`・`"skip"`・`"error"` | `OnUnknown::Skip` か `OnUnknown::Error`。何も表示せず、飛ばした種類は `SessionLog::unknown_types` に入る |
| ヘッダの未知の最上位のキー | 警告し、`metadata` に残す | `metadata` に残し、`unknown_keys` に並べる |
| `wall_clock_start` | 日時として読む | ファイルの文字列のまま持ち、検査しない |
| 行の区切り | Unicode の行の区切りを全部使う | 仕様どおり LF と CRLF だけ |
| 最後の行が文字の途中で切れている | 復号の誤りになる | 書きかけの最後の行と同じく捨てる |
| 整数 | 大きさの制限なし | 64 ビット。超える値は誤りになる |

MED-PC のファイルの読み込みは含まない。

## テスト

```bash
cargo test
```

適合テストは、親ディレクトリのどれかの隣にある OperantKitLog の checkout か、
`OKL_CONFORMANCE_ROOT` (`valid/` と `invalid/` を持つディレクトリ) からファイルを読む。
どちらも無ければ `SKIP` と表示して通る (`cargo test -- --nocapture` で見える)。

## ライセンス

MIT。[LICENSE](LICENSE) を参照。
