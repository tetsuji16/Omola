# Issue #15: Open Omola projects and libraries in OMEdit

GitHub: https://github.com/tetsuji16/Omola/issues/15

Depends on #13 and #14. Bridge the existing Omola manifest to OMEdit's normal model-opening workflow. OMEdit remains responsible for editing, annotations, library browsing, and model views.

## Acceptance

- Resolve manifest to canonical entry model and declared library roots using project validation; cover paths with spaces.
- Open the intended model in matching OMEdit by documented invocation or supported file handoff.
- Missing source/root/dependency or OMC/OMEdit version mismatch has an actionable diagnostic and never silently selects another system library version.
- Opening/browsing leaves project bytes unchanged; test path escape and missing root.
- Add no duplicate Omola editor/library/annotation UI or copied assets.

## Verification

Run workspace tests and clippy in Docker, validate examples/project.toml, then open a temporary BouncingBall copy in OMEdit and record resolved model/library and unchanged source hash.
