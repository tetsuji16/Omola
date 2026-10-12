# Issue #18: Measure OMEdit diagram support and Dymola workflow gaps

GitHub: https://github.com/tetsuji16/Omola/issues/18

Depends on #13, #15, and #16. Use OMEdit diagram/text layers and Modelica annotations by default; do not build a second graphical editor. Add only scoped Rust bridge or upstream contribution after reproducing a gap.

## Acceptance

- Lawful fixtures cover OMEdit rendering/editing of Modelica icon/diagram annotations and connections.
- Compare normalized annotations and verify unsupported annotation content is preserved on save.
- Matrix distinguishes Modelica language/library, OMEdit, Dymola GUI semantics, and proprietary services.
- Each Omola addition states trigger, supported/unsupported behavior, and evidence; no general Dymola graphical parity claim.
- Review upstream patch/fork technical and license obligations separately before adoption.

## Verification

Edit copied OMEdit fixtures, run OMC check, compare source/annotation diffs, and run bridge tests on supported OS.
