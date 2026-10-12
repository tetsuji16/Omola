# Issue #19: Add OMEdit workflow compatibility and release regression gates

GitHub: https://github.com/tetsuji16/Omola/issues/19

Depends on #13–#17; #18 contributes separate diagram cases. Provide repeatable evidence for the OMEdit-based workflow, not a visual similarity score.

## Acceptance

- Versioned report records OS, OMC/OMEdit versions, package origins, manifest/model hash, pass/fail/unsupported results.
- Fixture minimum and thresholds are explicit; stale manifests/reports and missing required cases fail the release gate.
- Automation covers launch/open/check/simulate/result/plot where display runners support it; manual evidence is labeled.
- Keyboard/focus/labels/contrast and startup/workflow latency are measured for the real OMEdit workflow when automatable.
- Scorecard separates Modelica, OMC, OMEdit, Omola, and Dymola-commercial dimensions; no parity inference from OMEdit success.

## Verification

Run release verifier, confirm changed manifest hash rejected, run available UI/backend jobs, and archive report with versions and unsupported cases.
