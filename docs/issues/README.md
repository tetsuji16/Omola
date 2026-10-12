# Omola issue briefs

These issues are tracked in [GitHub](https://github.com/tetsuji16/Omola/issues). Each local brief under this directory is the matching issue body. The repo currently has no project-specific `AGENTS.md`; issue work should follow the shared project instructions and preserve measured compatibility claims.

Recommended sequence: [01](https://github.com/tetsuji16/Omola/issues/1) -> [02](https://github.com/tetsuji16/Omola/issues/2) -> [03](https://github.com/tetsuji16/Omola/issues/3) -> [04](https://github.com/tetsuji16/Omola/issues/4), [05](https://github.com/tetsuji16/Omola/issues/5), [06](https://github.com/tetsuji16/Omola/issues/6), and [07](https://github.com/tetsuji16/Omola/issues/7) may proceed after the corpus contract is stable -> [08](https://github.com/tetsuji16/Omola/issues/8) -> [09](https://github.com/tetsuji16/Omola/issues/9) -> [10](https://github.com/tetsuji16/Omola/issues/10).

| Issue | Title | Depends on | Local brief |
| --- | --- | --- | --- |
| [#1](https://github.com/tetsuji16/Omola/issues/1) | Harden and document the OMC process adapter | — | [01](01-omc-process-adapter.md) |
| [#2](https://github.com/tetsuji16/Omola/issues/2) | Add a checked-in smoke model and reproducible CLI examples | [#1](https://github.com/tetsuji16/Omola/issues/1) | [02](02-smoke-model.md) |
| [#3](https://github.com/tetsuji16/Omola/issues/3) | Define a versioned compatibility corpus and report format | [#1](https://github.com/tetsuji16/Omola/issues/1), [#2](https://github.com/tetsuji16/Omola/issues/2) | [03](03-compatibility-corpus.md) |
| [#4](https://github.com/tetsuji16/Omola/issues/4) | Measure standard language and public-library coverage | [#3](https://github.com/tetsuji16/Omola/issues/3) | [04](04-language-library-coverage.md) |
| [#5](https://github.com/tetsuji16/Omola/issues/5) | Inventory Dymola scripting and implement a small compatibility bridge | [#3](https://github.com/tetsuji16/Omola/issues/3) | [05](05-scripting-bridge.md) |
| [#6](https://github.com/tetsuji16/Omola/issues/6) | Add result artifact discovery and CSV interchange | [#3](https://github.com/tetsuji16/Omola/issues/3) | [06](06-results-interchange.md) |
| [#7](https://github.com/tetsuji16/Omola/issues/7) | Add an FMI workflow through OpenModelica | [#3](https://github.com/tetsuji16/Omola/issues/3) | [07](07-fmi-workflow.md) |
| [#8](https://github.com/tetsuji16/Omola/issues/8) | Specify a basic model project and editor workflow | [#3](https://github.com/tetsuji16/Omola/issues/3) | [08](08-project-workflow.md) |
| [#9](https://github.com/tetsuji16/Omola/issues/9) | Audit library provenance, license constraints, and extensions | [#3](https://github.com/tetsuji16/Omola/issues/3) | [09](09-provenance-licenses.md) |
| [#10](https://github.com/tetsuji16/Omola/issues/10) | Publish compatibility scorecard and release gates | [#4](https://github.com/tetsuji16/Omola/issues/4)–[#9](https://github.com/tetsuji16/Omola/issues/9) | [10](10-release-scorecard.md) |
