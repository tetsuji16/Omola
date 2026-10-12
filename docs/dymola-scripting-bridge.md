# Dymola scripting bridge inventory

This is a small, versioned-by-repository inventory of observed mappings, not a Dymola API reimplementation. No user-authorized Dymola scripts were supplied for this task, so the inventory is based on the published documentation linked below. `omola bridge inventory` emits the current function counts as JSON or Markdown. `omola bridge call` accepts only the explicitly listed function names and returns a capability-specific error for unavailable functions or parameters.

| Dymola function | Mapping | Omola operation | Fixture evidence | Limitation |
| --- | --- | --- | --- | --- |
| `simulateModel` | Partial | `omola simulate --model FILE.mo --class Modelica.Class [--stop-time SECONDS] [--output PREFIX]` | `bouncing-ball-simulate`, `msl-first-order-blocks` | Only file, class, stop time, and output prefix are mapped. Start time, integration method, tolerance, interval controls, parameter/start-value overrides, exact result filename semantics, and persistent session state are unsupported. Unsupported options such as `--method` fail with a parameter-specific diagnostic. |
| `translateModel` | Unsupported | None | None | No standalone translation command or generated-code artifact is exposed. OMC translation currently occurs within simulation. |
| `plot` | Unsupported | None | None | No interactive plotting or image export is available. |
| `getLastError` | Unsupported | None | None | Omola reports diagnostics with the operation that produced them; it has no persistent simulator session or stateful last-error query. |

The evidence fixtures use original MIT-licensed Omola models and record observable check/simulation outcomes and result artifacts through the OpenModelica backend. A partial mapping means only the listed parameters and observations are covered; it does not imply semantic equivalence for omitted parameters or Dymola-specific extensions. No Dymola source, scripts, proprietary library, or copied manual text is included.

The inventory is based on these published Dymola documentation pages:

- [Python Interface: Quick Reference](https://help-3dexperience.aesvietnam.com/English/DbmUserMap/dbm-r-PythonInterface-QuickReference.htm) documents `simulateModel`, `translateModel`, their inputs/results, and `getLastError` behavior.
- [About the DymolaCommands Library and Use of Commands](https://help-3dexperience.aesvietnam.com/English/DbmUserMap/svefordbm-c-DymolaCommands-About.htm) separates simulator, plot, trajectory, and other command groups and documents Dymola-specific command usage.
- [JavaScript Interface](https://help-3dexperience.aesvietnam.com/English/DbmUserMap/dbm-c-JavascriptInterface.htm) describes the Dymola interface object and examples using simulation and plotting calls.

Dymola's scripting service is part of its commercial tool environment. Omola's bridge has no Dymola runtime, license, GUI, or vendor-library dependency: supported work is delegated to the separately installed OpenModelica compiler. This scope does not cover scripts requiring Dymola's session, GUI, proprietary libraries, or product-specific integrations.
