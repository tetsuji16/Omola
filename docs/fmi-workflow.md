# FMI workflow

Omola delegates FMU creation and experimental import to the separately installed OpenModelica Compiler. The current `fmi capabilities` output is a documentation profile pinned to OMC 1.27.1, not a live probe. OpenModelica's [1.27.1 FMI guide](https://openmodelica.org/doc/OpenModelicaUsersGuide/1.27/fmitlm.html) documents `buildModelFMU`, FMI 1.0 Model Exchange, and FMI 2.0.4 Model Exchange/Co-Simulation. The selector passed to OMC is `2.0`. For a different installed OMC version the command warns that its capabilities were not probed.

`fmi export` validates the source/class, version selector, interface type, and output extension, calls `buildModelFMU` in a temporary OMC working directory, and copies only a real generated archive to a non-existing destination. The file-name prefix uses Modelica/C-identifier-safe characters because OMC otherwise can generate mismatched C and header names. On success Omola validates a ZIP archive with `modelDescription.xml`, a `binaries/` entry, an FMI version, model name, and Model Exchange or Co-Simulation declaration, then writes an adjacent JSON metadata sidecar with source, class, requested FMI version/type, OMC version, host platform, and export time.

`fmi import --file FILE.fmu --output-dir NEW_DIRECTORY` accepts FMI 2.0 Model Exchange FMUs and calls OMC `importFMU` with the `--allowNonStandardModelica=reinitInAlgorithms` flag. It requires a new output directory, records the generated Modelica wrapper path and import limitations, and preserves OMC's generated files. Import remains experimental; this command does not simulate the imported wrapper or check numerical equivalence. FMI 2.0 Co-Simulation and FMI 1.0 are rejected by this import path.

`fmi validate FILE.fmu` checks archive structure and basic metadata only. It does not run the FMU, prove FMI conformance, compare numerical behavior, or establish Dymola parity.

On the pinned development container, the verified commands are:

```sh
cargo run -p omola -- fmi capabilities
cargo run -p omola -- fmi export --model examples/BouncingBall.mo --class BouncingBall --version 2.0 --type cs --output target/BouncingBall.fmu
cargo run -p omola -- fmi validate target/BouncingBall.fmu
cargo run -p omola -- fmi export --model examples/BouncingBall.mo --class BouncingBall --version 2.0 --type me --output target/BouncingBallME.fmu
cargo run -p omola -- fmi import --file target/BouncingBallME.fmu --output-dir target/BouncingBall-import
```

`BouncingBall` is an original Omola MIT-licensed model. The exported artifact is built locally from the installed OMC; no FMU binary is committed to the repository.
