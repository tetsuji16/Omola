# OMEdit workflow evidence

`omedit-evidence.json` is the versioned baseline for the bounded compatibility corpus. Its `utf-8-crlf-to-lf-v1` hash format computes SHA-256 over UTF-8 text after replacing CRLF pairs with LF; lone CR characters stay unchanged. This makes manifest and model digests stable across Windows and Linux checkouts without claiming to hash the raw working-tree bytes. The verifier also confines resolved model paths to this repository workspace and rejects duplicate, missing, or unknown fixture IDs.

Run the normal verification from the repository root:

```sh
cargo run -p omola -- omedit evidence verify \
  --evidence docs/omedit-evidence.json \
  --manifest tests/compat/manifest.toml
```

Normal verification reports `PASS`, `FAIL`, and `UNVERIFIED` counts without converting unmeasured UI behavior into a failure or success claim. Release verification adds `--require-verified`; each non-skipped manifest fixture must then be `PASS`, and OS, exact OMC/OMEdit versions and package origins must all be measured rather than `unverified`. Update the evidence only after a real run, recording the evidence type and a concise observation. For UI checks, label manually inspected evidence `manual`. A hash mismatch means the report must be re-measured, not edited to copy a new digest.

The current baseline records the official Ubuntu 22.04 OpenModelica 1.27.1 package pair described in [omedit-support.md](omedit-support.md). OMEdit process launch and container readiness were observed; visible editor contents, GUI check/simulation, result plotting, keyboard/focus/contrast, and Dymola parity remain `UNVERIFIED`. The report is evidence bookkeeping, not a visual-similarity score or a Dymola compatibility claim.
