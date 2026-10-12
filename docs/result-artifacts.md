# Result artifacts

`omola simulate` writes the result file produced by the selected OpenModelica Compiler and an adjacent JSON sidecar (`<result-stem>.json`). The versioned sidecar records Unix-millisecond start/completion times, Omola/OMC versions, source path and class, result path, and the solver-options text reported by OMC. Parameter overrides are not available in the current CLI, so `parameters` is an empty list; units are not reported by this adapter.

`omola results inspect FILE.csv` accepts UTF-8 CSV with a `time` first column and at least one variable. It rejects duplicate/empty headers, blank cells, non-finite/non-numeric data, uneven rows, and decreasing time. Equal adjacent times are allowed for solver event points. `omola results inspect FILE.mat` delegates header and variable discovery to OMC. Other extensions, including `.dsres`, are rejected with an explicit unsupported-format diagnostic.

`omola results export-csv FILE.mat --variables x,y --output FILE.csv` reads selected scalar result variables through the OMC scripting API and writes `time` plus those variables. OMC 1.27.1's scripting `String` formatter can truncate 16+ significant digits for event values (fixed internal buffer); the adapter requests 13 significant digits to avoid emitting empty/corrupted values. CSV parsing and formatting preserve full Rust `f64` round-trip precision for CSV values already available to the adapter. Units are not inferred or synthesized.

This is OpenModelica-specific result handling. Omola does not parse Dymola MAT/DSRES files independently and makes no Dymola result-file compatibility claim. A future claim requires independently sourced fixtures, variable/time coverage, units, tolerances, and explicit version scope.
