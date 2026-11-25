"""Write tests/fixtures/float_repr.tsv: the bits of a float and Python's repr of it.

Run with any CPython 3.1 or later; the values come from a fixed seed, so the
output does not change between runs:

    python3 scripts/gen_float_repr.py > tests/fixtures/float_repr.tsv
"""

import random
import struct

rng = random.Random(0)
values = [
    0.0, -0.0, 1.0, -1.0, 0.1, 0.5, 1.5, 600.0, 0.1 + 0.2, 1 / 3,
    1e15, 123456789012345.6, 9999999999999998.0, 1e16, 1e17, 1.2345678901234568e17, 1e22, 1e23, 1e100,
    1e-3, 1e-4, 0.00012345, 9.999e-5, 1e-5, 1.5e-7, 1e-10, 1e-99, 1e-100,
    5e-324, 2.2250738585072014e-308, 1.7976931348623157e308,
    float("inf"), float("-inf"), float("nan"),
    # Halfway between two shortest spellings: Python takes the even digit.
    1148024620201471.25, 1148024620201471.75, -111275153569243.125, 2056655888558458.25,
    # The nearest spelling of that length is another float.
    2.0**-44, 2.0**-1074 * 2, 9007199254740992.0, 2.0**89,
]
while len(values) < 100:
    values.append(rng.uniform(0.0, 3600.0))
while len(values) < 160:
    values.append(rng.uniform(-1.0, 1.0) * 10.0 ** rng.randint(-8, 20))
while len(values) < 300:
    v = struct.unpack("<d", struct.pack("<Q", rng.getrandbits(64)))[0]
    if v == v:
        values.append(v)
for v in values:
    bits = struct.unpack("<Q", struct.pack("<d", v))[0]
    print(f"{bits:016x}\t{v!r}")
