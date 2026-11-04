# Issue #20: Main-screen GUI Dymola compatibility plan

GitHub: (planning document; tracks #25 #15 #16 #17 #18 #19 #22)

## Goal

Make Omola's primary graphical modeling surface (main screen) reach measured
Dymola-equivalent workflows for model text/diagram editing, package/project
browsing, check/simulation, simulation settings, results/plotting, and diagram
annotations, while keeping Dymola-only/commercial features explicitly out of
scope until a lawful, measured mapping exists.

Omola does not copy Dymola code, manuals, models, icons, or proprietary UI
assets. Dymola public feature summaries may guide a gap matrix but are not a
specification.

## Non-goals

- No claim of Dymola parity until each dimension has measured evidence.
- No proprietary Dymola graphical extensions, parameter sweep/calibration/
  optimization, or vendor services.
- No in-process plugin claim; OMEdit is patched as a small reviewed patch
  series against pinned upstream source.

## Stages (dependency order)

1. #25 Pinned native Windows OMEdit source build + license/dependency record.
   Exit: isolated OMEdit+OMC build launches on Windows; fixture check/simulate.
2. #15 Native "Open Omola Project..." action + model/library loading.
   Exit: GUI action consumes validated v1 JSON, opens canonical model, preserves
   source, reports missing inputs.
3. #16 Manifest simulation settings inside OMEdit.
   Exit: stop time + numeric overrides visible and applied to real OMC run;
   GUI diagnostics/results agree.
4. #17 Result browsing and plotting.
   Exit: OMEdit renders fixture variables with reported result path/backend;
   no synthetic plot evidence.
5. #18 Diagram annotations and measured Dymola gaps.
   Exit: lawful fixtures, annotation-preserving saves, supported/unsupported matrix.
6. #22 Verify versions selected in OMEdit's own OMC session.
   Exit: conflicting pins/annotations fail visibly; version evidence from GUI.
7. #19 Native Windows GUI regression gate.
   Exit: versioned open/check/simulate/plot report + explicit unsupported cases.

## Dymola compatibility dimensions (main screen)

| Area | Baseline | Target status | Evidence |
|---|---|---|---|
| Text/diagram editing | OMEdit editors reused | reused (measured save round-trip) | #18 |
| Package/project browsing | OMEdit browser + manifest | partial -> measured | #15 #22 |
| Check/simulation | OMEdit actions + OMC | partial -> measured | #16 |
| Simulation settings handoff | not-measured | measured | #16 |
| Results/plotting | OMEdit browser/plots | reused (measured) | #17 |
| Diagram annotations/connections | not-measured | measured/unsupported matrix | #18 |
| Native regression gate | not-measured | measured | #19 |
| Dymola-specific GUI semantics | unsupported | unsupported (explicit) | n/a |

## Current blockers

- Native OMEdit source build (#25) incomplete: CMake configure not finished;
  OMDev/UCRT64 download stalled; no modified C++ compiled.
- Stock OMEdit does not receive manifest simulation settings (#16).
- Library version selected by GUI session unverified (#22).

## Verification

- Run `omola gui status --format json` for measured baseline (must not claim parity).
- Run `git diff --check`.
- Each stage's acceptance requires its own measured evidence; an issue stays open
  until then.
