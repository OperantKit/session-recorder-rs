"""Check that the Python session-recorder reads what this crate writes.

Needs `cargo` and a Python with session-recorder installed:

    python scripts/check_python_roundtrip.py [LOG ...]

Each log (by default the ones in tests/fixtures/okl) is rewritten by the
`rewrite_okl` example; Python must read the same values from both files.
"""

import subprocess
import sys
import tempfile
from pathlib import Path

from session_recorder import read_log

root = Path(__file__).resolve().parent.parent
logs = [Path(p) for p in sys.argv[1:]] or sorted((root / "tests/fixtures/okl").glob("*.txt"))
failed = False
with tempfile.TemporaryDirectory() as tmp:
    for log in logs:
        out = Path(tmp) / log.name
        subprocess.run(
            ["cargo", "run", "-q", "--example", "rewrite_okl", "--", str(log.resolve()), str(out)],
            cwd=root,
            check=True,
        )
        before, after = read_log(log, on_unknown="error"), read_log(out, on_unknown="error")
        same_bytes = log.read_bytes() == out.read_bytes()
        ok = before == after
        failed |= not ok
        print(
            f"{'ok  ' if ok else 'FAIL'} {log.name}: {len(after.events)} events, "
            f"{len(after.raw_records)} records, {len(after.phase_markers)} phase markers, "
            f"bytes {'same' if same_bytes else 'differ'}"
        )
sys.exit(failed)
