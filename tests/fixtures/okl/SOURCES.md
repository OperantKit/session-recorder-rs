# Sources of the logs in this directory

Written by the Python `operant` command (operantkit-cli, virtual subject) and,
for `phases.txt`, by a script that puts the public Python packages together.
The subjects are simulated with fixed seeds; no recorded data is used.

| File | Command or script |
|---|---|
| `fr3.txt` | `operant run --schedule "FR3" --exit-reinforcers 5 --virtual-rate-hz 5.0 --seed 1` |
| `conc.txt` | `operant run --schedule 'Conc(VI 30-s, VI 60-s, COD=2-s) @algorithm("fleshler-hoffman", seed=3)' --exit-time 600 --virtual-rate-hz 0.5 --seed 4` |
| `phases.txt` | Three phases with one virtual subject (1.0 response/s, seed 2) and one clock: VI 10-s (seed 7) for 5 reinforcers in context A with a houselight, EXT for 60 s in context B, EXT for 30 s in context A with a houselight |
