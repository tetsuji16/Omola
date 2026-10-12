# Issue #13: Define OMEdit reuse, version, and licensing boundary

GitHub: https://github.com/tetsuji16/Omola/issues/13

## Scope

Use the official OpenModelica OMEdit GUI as Omola's primary graphical modeling environment. Inventory the exact OMC/OMEdit version pairing, package/source origin, licenses for distributed components, supported invocation, and extension/embedding boundary. The upstream source header for OMEdit permits AGPL-3.0 or OSMC-PL 1.8 at recipient choice; this is not by itself a license inventory for Qt and every dependency. No assets or source are copied.

## Acceptance

- A versioned table names tested OS, OpenModelica and OMEdit versions, distribution source, and package/license identifiers.
- Official source headers and license files for OMEdit and relevant packaged dependencies are linked/referenced; license selection and redistribution consequences are recorded for maintainers.
- The plan states that no supported in-process plugin/embedding API has been established; any bridge claim cites an official interface and a working probe.
- OMEdit's supported editor, diagram, library, messages, simulation, and plot capabilities are mapped to Omola stages; Dymola-only and commercial features remain explicitly unsupported/unknown.
- No custom GUI or copied OMEdit/Dymola asset is introduced by this issue.

## Verification

Check the cited official source at the pinned tag/commit, compare package metadata, validate all roadmap links and issue dependencies, and run `git diff --check`.
