# Issue #17: Reuse OMEdit result browsing and plotting

GitHub: https://github.com/tetsuji16/Omola/issues/17

Depends on #16 and existing result interchange (#6). Prefer OMEdit's variable browser/plot perspective; Rust handles only measured inspection/conversion gaps.

## Acceptance

- Real fixture simulation creates a result that OMEdit opens and plots; record exact tool versions and artifact path.
- Verify variable discovery, scalar selection, time axis, legend, and rendered plot.
- Omola result metadata agrees with actual result path/backend; advertise only supported formats.
- State units/format limits accurately; do not claim Dymola MAT/DSRES support.
- Test missing/corrupt results and unknown variables; synthetic plots do not count.

## Verification

Run workspace tests, existing results inspect/export-csv checks, and a real OMEdit fixture plot smoke.
