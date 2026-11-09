//! Process boundary around an installed OpenModelica Compiler (OMC).
//! Modelica parsing, translation, and simulation remain the external backend's job.

pub mod mat;

use std::{
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub enum BackendError {
    MissingCompiler {
        searched: String,
    },
    InvalidConfiguredCompiler {
        path: PathBuf,
    },
    LaunchFailed {
        executable: PathBuf,
        source: io::Error,
    },
    Io(io::Error),
    InvalidInput(String),
    CompilerFailed {
        version: String,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    CompilerDiagnostic {
        version: String,
        output: String,
    },
    InvalidCompilerResponse {
        version: String,
        output: String,
    },
    Cancelled,
}

pub struct SimulationConfig<'a> {
    pub stop_time: f64,
    /// `startTime=` in `simulate(...)`; `None` keeps the OMC default.
    pub start_time: Option<f64>,
    /// `numberOfIntervals=` in `simulate(...)`; `None` keeps the OMC default.
    pub number_of_intervals: Option<u32>,
    /// `tolerance=` in `simulate(...)`; `None` keeps the OMC default.
    pub tolerance: Option<f64>,
    /// `method=` in `simulate(...)`; must be one of [`SIMULATION_METHODS`].
    pub method: Option<&'a str>,
    pub result: Option<&'a Path>,
    pub libraries: &'a [(String, String)],
    pub package_roots: &'a [PathBuf],
    pub parameters: &'a [(String, String)],
}

/// Integration methods accepted by OpenModelica 1.27.1.
///
/// This is the list OMC itself prints when `simulate(..., method="bogus")` fails
/// ("current options are: ..."), minus the entries that do not run without extra
/// configuration: `symSolver` is reported as deprecated and `optimization` needs a
/// special setup, so neither is offered. Every name here was verified to complete a
/// simulation of `examples/BouncingBall.mo` with OMC 1.27.1.
pub const SIMULATION_METHODS: &[&str] = &[
    "dassl",
    "ida",
    "cvode",
    "gbode",
    "euler",
    "rungekutta",
    "symSolverSsc",
    "qss",
];

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCompiler { searched } => write!(f, "OpenModelica compiler `omc` was not found ({searched}). Install it with scripts/setup-windows.ps1, scripts/setup-linux.sh or scripts/setup-macos.sh (see docs/install.md), add its bin directory to PATH, or set OMOLA_OMC to the executable path."),
            Self::InvalidConfiguredCompiler { path } => write!(f, "OMOLA_OMC points to a missing or non-file path: {}", path.display()),
            Self::LaunchFailed { executable, source } => write!(f, "Could not launch OpenModelica at {}: {source}", executable.display()),
            Self::Io(source) => write!(f, "OMC adapter I/O failed: {source}"),
            Self::InvalidInput(message) => write!(f, "Invalid request: {message}"),
            Self::CompilerFailed { version, status, stdout, stderr } => write!(f, "OpenModelica {version} exited with status {status:?}.\nstdout:\n{stdout}\nstderr:\n{stderr}"),
            Self::CompilerDiagnostic { version, output } => write!(f, "OpenModelica {version} reported an error:\n{output}"),
            Self::InvalidCompilerResponse { version, output } => write!(f, "OpenModelica {version} returned no usable result:\n{output}"),
            Self::Cancelled => write!(f, "Operation cancelled by the user."),
        }
    }
}
impl std::error::Error for BackendError {}
impl From<io::Error> for BackendError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub omc_version: String,
    pub output: String,
    pub result_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Omc {
    executable: PathBuf,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Omc {
    /// Resolve the compiler: a non-empty `OMOLA_OMC` first, then `omc[.exe]` on PATH, then
    /// the standard OpenModelica installation locations (`OPENMODELICAHOME`, and on Windows
    /// `C:\Program Files\OpenModelica*\bin`, newest version first).
    pub fn discover() -> Result<Self, BackendError> {
        if let Some(configured) = env::var_os("OMOLA_OMC") {
            if !configured.is_empty() {
                let path = PathBuf::from(configured);
                if !path.is_file() {
                    return Err(BackendError::InvalidConfiguredCompiler { path });
                }
                return Ok(Self {
                    executable: path,
                    cancel: None,
                });
            }
        }
        let name = if cfg!(windows) { "omc.exe" } else { "omc" };
        if let Some(path) = find_on_path(name) {
            return Ok(Self {
                executable: path,
                cancel: None,
            });
        }
        let program_files = if cfg!(windows) {
            Some(
                env::var_os("ProgramFiles")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("C:\\Program Files")),
            )
        } else {
            None
        };
        let installed = installation_candidates(
            env::var_os("OPENMODELICAHOME").map(PathBuf::from),
            program_files.as_deref(),
            name,
        );
        if let Some(path) = installed.into_iter().find(|p| p.is_file()) {
            return Ok(Self {
                executable: path,
                cancel: None,
            });
        }
        Err(BackendError::MissingCompiler {
            searched: "checked OMOLA_OMC, PATH, OPENMODELICAHOME and the standard install folders"
                .into(),
        })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn with_cancel_flag(mut self, flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    pub fn version(&self) -> Result<String, BackendError> {
        let out = self.invoke([OsString::from("--version")])?;
        let text = output_text(&out);
        if !out.status.success() {
            return Err(failed("unknown", out));
        }
        let version = text.trim().to_owned();
        if version.is_empty() {
            return Err(BackendError::InvalidCompilerResponse {
                version: "unknown".into(),
                output: text,
            });
        }
        Ok(version)
    }

    pub fn check_model(&self, source: &Path, class: &str) -> Result<CommandResult, BackendError> {
        self.check_model_with_libraries(source, class, &[])
    }

    pub fn check_model_with_libraries(
        &self,
        source: &Path,
        class: &str,
        libraries: &[(String, String)],
    ) -> Result<CommandResult, BackendError> {
        self.check_project_model(source, class, libraries, &[])
    }

    pub fn check_project_model(
        &self,
        source: &Path,
        class: &str,
        libraries: &[(String, String)],
        package_roots: &[PathBuf],
    ) -> Result<CommandResult, BackendError> {
        validate_class(class)?;
        let source = readable_model(source)?;
        let mut script = self.package_path_script(package_roots)?;
        script.push_str("clearMessages();\n");
        script.push_str(&load_libraries_script(libraries)?);
        script.push_str(&format!(
            "loaded := loadFile(\"{}\");\nif not loaded then print(getErrorString()); end if;\nif loaded then result := checkModel({}); print(result); print(getErrorString()); end if;\n",
            modelica_string(&source.to_string_lossy()), class
        ));
        let result = self.run_script(&script)?;
        if contains_false(&result.output) {
            return Err(BackendError::CompilerDiagnostic {
                version: result.omc_version,
                output: result.output,
            });
        }
        Ok(result)
    }

    pub fn simulate(
        &self,
        source: &Path,
        class: &str,
        stop_time: f64,
        result: Option<&Path>,
    ) -> Result<CommandResult, BackendError> {
        self.simulate_with_libraries(source, class, stop_time, result, &[])
    }

    pub fn simulate_with_libraries(
        &self,
        source: &Path,
        class: &str,
        stop_time: f64,
        result: Option<&Path>,
        libraries: &[(String, String)],
    ) -> Result<CommandResult, BackendError> {
        self.simulate_project_model(source, class, stop_time, result, libraries, &[])
    }

    pub fn simulate_project_model(
        &self,
        source: &Path,
        class: &str,
        stop_time: f64,
        result: Option<&Path>,
        libraries: &[(String, String)],
        package_roots: &[PathBuf],
    ) -> Result<CommandResult, BackendError> {
        self.simulate_project_model_with_parameters(
            source,
            class,
            SimulationConfig {
                stop_time,
                start_time: None,
                number_of_intervals: None,
                tolerance: None,
                method: None,
                result,
                libraries,
                package_roots,
                parameters: &[],
            },
        )
    }

    pub fn simulate_project_model_with_parameters(
        &self,
        source: &Path,
        class: &str,
        config: SimulationConfig<'_>,
    ) -> Result<CommandResult, BackendError> {
        self.simulate_class(Some(source), class, config)
    }

    /// Simulate `class`. With `source: None` the class must come from `config.libraries`
    /// (for example `Modelica.Blocks.Examples.PID_Controller`); no user file is loaded.
    pub fn simulate_class(
        &self,
        source: Option<&Path>,
        class: &str,
        config: SimulationConfig<'_>,
    ) -> Result<CommandResult, BackendError> {
        let SimulationConfig {
            stop_time,
            result,
            libraries,
            package_roots,
            parameters,
            ..
        } = config;
        validate_class(class)?;
        validate_parameter_overrides(parameters)?;
        let source = source.map(readable_model).transpose()?;
        if source.is_none() && libraries.is_empty() {
            return Err(BackendError::InvalidInput(
                "a model file or at least one library is required to simulate".into(),
            ));
        }
        if !stop_time.is_finite() || stop_time < 0.0 {
            return Err(BackendError::InvalidInput(
                "stop time must be finite and non-negative".into(),
            ));
        }
        let prefix = match result {
            Some(path) => absolute_output(path)?,
            None => env::current_dir()?.join(unique_name("result")),
        };
        let result_path = prefix.with_file_name(format!(
            "{}_res.mat",
            prefix.file_name().unwrap_or_default().to_string_lossy()
        ));
        if prefix.exists() || result_path.exists() {
            return Err(BackendError::InvalidInput(format!(
                "simulation output already exists for prefix: {}",
                prefix.display()
            )));
        }
        if let Some(parent) = result_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                BackendError::Io(io::Error::new(
                    error.kind(),
                    format!(
                        "could not create simulation output directory {}: {error}",
                        parent.display()
                    ),
                ))
            })?;
        }
        let output_prefix = unique_name("simulation");
        let mut script = self.package_path_script(package_roots)?;
        script.push_str("clearMessages();\n");
        script.push_str(&load_libraries_script(libraries)?);
        let simflags = if parameters.is_empty() {
            String::new()
        } else {
            format!(
                "-override={}",
                parameters
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        if let Some(source) = &source {
            script.push_str(&format!(
                "loaded := loadFile(\"{}\");\nif not loaded then print(getErrorString()); end if;\n",
                modelica_string(&source.to_string_lossy())
            ));
        }
        script.push_str(&simulate_call(class, &output_prefix, &config, &simflags)?);
        let (mut result, workspace, _cleanup) = self.run_script_in_temp(&script)?;
        let generated = match find_result_file(&result.output) {
            Some(path) if path.is_absolute() => path,
            Some(path) => workspace.join(path),
            None => {
                return Err(BackendError::InvalidCompilerResponse {
                    version: result.omc_version,
                    output: result.output,
                })
            }
        };
        if !generated.is_file() {
            return Err(BackendError::InvalidCompilerResponse {
                version: result.omc_version,
                output: result.output,
            });
        }
        let mut source_file = fs::File::open(&generated)?;
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&result_path)
            .map_err(|error| {
                BackendError::Io(io::Error::new(
                    error.kind(),
                    format!(
                        "could not create simulation result {}: {error}",
                        result_path.display()
                    ),
                ))
            })?;
        if let Err(error) = io::copy(&mut source_file, &mut destination) {
            let _ = fs::remove_file(&result_path);
            return Err(BackendError::Io(io::Error::new(
                error.kind(),
                format!(
                    "could not copy simulation result to {}: {error}",
                    result_path.display()
                ),
            )));
        }
        result.output = result.output.replace(
            &generated.to_string_lossy().to_string(),
            &result_path.to_string_lossy(),
        );
        result.output = result.output.replace(
            &generated
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            &result_path.to_string_lossy(),
        );
        result.result_file = Some(result_path);
        Ok(result)
    }

    /// Export one FMU through OMC and copy the resulting archive without overwriting.
    pub fn export_fmu(
        &self,
        source: &Path,
        class: &str,
        version: &str,
        fmu_type: &str,
        output: &Path,
    ) -> Result<CommandResult, BackendError> {
        validate_class(class)?;
        let source = readable_model(source)?;
        if !matches!(version, "1.0" | "2.0") {
            return Err(BackendError::InvalidInput(
                "supported export version selectors are 1.0 and 2.0; consult installed OMC capabilities".into(),
            ));
        }
        if !matches!(fmu_type, "me" | "cs" | "me_cs") {
            return Err(BackendError::InvalidInput(
                "FMU type must be me, cs, or me_cs".into(),
            ));
        }
        if version == "1.0" && fmu_type != "me" {
            return Err(BackendError::InvalidInput(
                "the documented FMI 1.0 export profile supports Model Exchange only".into(),
            ));
        }
        if output.extension().and_then(|extension| extension.to_str()) != Some("fmu") {
            return Err(BackendError::InvalidInput(
                "FMU output path must use the .fmu extension".into(),
            ));
        }
        let output = absolute_output(output)?;
        if output.exists() {
            return Err(BackendError::InvalidInput(format!(
                "refusing to overwrite existing FMU: {}",
                output.display()
            )));
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        // OMC emits C identifiers from the prefix; hyphens can make its
        // generated .c/.h names disagree and silently prevent FMU creation.
        let prefix = unique_name("fmu").replace('-', "_");
        let script = format!(
            "clearMessages();\nloaded := loadFile(\"{}\");\nif not loaded then print(\"OMOLA_FMI_LOAD_FAILED\\n\" + getErrorString()); end if;\nif loaded then fmu := buildModelFMU({}, version=\"{}\", fmuType=\"{}\", fileNamePrefix=\"{}\"); print(\"OMOLA_FMI_FILE:\" + fmu + \"\\n\"); end if;\n",
            modelica_string(&source.to_string_lossy()),
            class,
            version,
            fmu_type,
            prefix
        );
        let (mut result, workspace, _cleanup) = self.run_script_in_temp(&script)?;
        let generated = workspace.join(format!("{prefix}.fmu"));
        if result.output.contains("OMOLA_FMI_LOAD_FAILED") || !generated.is_file() {
            return Err(BackendError::InvalidCompilerResponse {
                version: result.omc_version,
                output: result.output,
            });
        }
        let mut source_file = fs::File::open(&generated)?;
        let mut destination_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        if let Err(error) = io::copy(&mut source_file, &mut destination_file) {
            let _ = fs::remove_file(&output);
            return Err(BackendError::Io(error));
        }
        result.output = result.output.replace(
            &generated.to_string_lossy().to_string(),
            &output.to_string_lossy(),
        );
        result.result_file = Some(output);
        Ok(result)
    }

    /// Import an FMU as a generated Modelica wrapper into a new directory.
    pub fn import_fmu(
        &self,
        archive: &Path,
        output_dir: &Path,
    ) -> Result<CommandResult, BackendError> {
        let archive = archive.canonicalize()?;
        if !archive.is_file() {
            return Err(BackendError::InvalidInput(format!(
                "FMU archive is not a file: {}",
                archive.display()
            )));
        }
        let output_dir = absolute_output(output_dir)?;
        if output_dir.exists() {
            return Err(BackendError::InvalidInput(format!(
                "refusing to write imported FMU into an existing directory: {}",
                output_dir.display()
            )));
        }
        if let Some(parent) = output_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(&output_dir)?;
        let script = format!(
            "clearMessages();\nsetCommandLineOptions(\"--allowNonStandardModelica=reinitInAlgorithms\");\nimported := importFMU(\"{}\", workdir=\"{}\", fullPath=true);\nprint(\"OMOLA_IMPORTED_FMU:\" + imported + \"\\n\");\n",
            modelica_string(&archive.to_string_lossy()),
            modelica_string(&output_dir.to_string_lossy())
        );
        let mut result = self.run_script(&script)?;
        let generated = result
            .output
            .lines()
            .find_map(|line| line.trim().strip_prefix("OMOLA_IMPORTED_FMU:"))
            .map(str::trim)
            .filter(|path| !path.is_empty() && *path != "\"\"")
            .map(PathBuf::from)
            .ok_or_else(|| BackendError::InvalidCompilerResponse {
                version: result.omc_version.clone(),
                output: result.output.clone(),
            })?;
        if !generated.is_file() {
            return Err(BackendError::InvalidCompilerResponse {
                version: result.omc_version,
                output: result.output,
            });
        }
        result.result_file = Some(generated);
        Ok(result)
    }

    /// Run a script and return versioned command output. Scratch scripts are deleted on every exit.
    pub fn run_script(&self, script: &str) -> Result<CommandResult, BackendError> {
        self.run_script_in_temp(script).map(|(result, _, _)| result)
    }

    /// Run a read-only query script whose output may legitimately contain the text `Error:`
    /// (for example documentation HTML). Only a failed process is an error; the caller must
    /// interpret the output itself, typically through explicit markers it printed.
    pub fn run_query_script(&self, script: &str) -> Result<CommandResult, BackendError> {
        let version = self.version()?;
        let cleanup = TempDir::new("query")?;
        let path = cleanup.path.join("query.mos");
        let script = format!(
            "cd(\"{}\");\n{script}",
            modelica_string(&cleanup.path.to_string_lossy())
        );
        fs::write(&path, script)?;
        let out = self.invoke_in([path.as_os_str().to_owned()], Some(&cleanup.path))?;
        let text = output_text(&out);
        if !out.status.success() {
            return Err(failed(&version, out));
        }
        Ok(CommandResult {
            omc_version: version,
            output: text,
            result_file: None,
        })
    }

    /// Return the current project-safe Modelica path, prepending validated roots while
    /// retaining OpenModelica's installation defaults and user library directories.
    fn package_path_script(&self, roots: &[PathBuf]) -> Result<String, BackendError> {
        if roots.is_empty() {
            return Ok(String::new());
        }
        let current = self.run_script("print(\"OMOLA_MODELICAPATH:\" + getModelicaPath());\n")?;
        let current_path = current
            .output
            .lines()
            .find_map(|line| line.strip_prefix("OMOLA_MODELICAPATH:"))
            .ok_or_else(|| BackendError::InvalidCompilerResponse {
                version: current.omc_version.clone(),
                output: current.output.clone(),
            })?;
        let mut paths = roots.to_vec();
        paths.extend(env::split_paths(std::ffi::OsStr::new(current_path)));
        let joined = env::join_paths(paths).map_err(|error| BackendError::InvalidInput(format!("could not combine project package roots with OpenModelica's current library path: {error}")))?;
        let joined = joined.to_string_lossy();
        Ok(format!(
            "if not setModelicaPath(\"{}\") then print(getErrorString()); end if;\n",
            modelica_string(&joined)
        ))
    }

    fn run_script_in_temp(
        &self,
        script: &str,
    ) -> Result<(CommandResult, PathBuf, TempDir), BackendError> {
        let version = self.version()?;
        let cleanup = TempDir::new("command")?;
        let path = cleanup.path.join("command.mos");
        let script = format!(
            "cd(\"{}\");\n{script}",
            modelica_string(&cleanup.path.to_string_lossy())
        );
        fs::write(&path, script)?;
        let out = self.invoke_in([path.as_os_str().to_owned()], Some(&cleanup.path))?;
        let text = output_text(&out);
        if !out.status.success() {
            return Err(failed(&version, out));
        }
        if has_error_diagnostic(&text) {
            return Err(BackendError::CompilerDiagnostic {
                version,
                output: text,
            });
        }
        Ok((
            CommandResult {
                omc_version: version,
                output: text,
                result_file: None,
            },
            cleanup.path.clone(),
            cleanup,
        ))
    }

    fn invoke<I, S>(&self, args: I) -> Result<Output, BackendError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.invoke_in(args, None)
    }

    fn invoke_in<I, S>(&self, args: I, current_dir: Option<&Path>) -> Result<Output, BackendError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = Command::new(&self.executable);
        command.args(args);
        if let Some(dir) = current_dir {
            command.current_dir(dir);
        }
        let launch_failed = |source| BackendError::LaunchFailed {
            executable: self.executable.clone(),
            source,
        };
        let Some(flag) = &self.cancel else {
            return command.output().map_err(launch_failed);
        };
        use std::io::Read;
        use std::process::Stdio;
        use std::sync::atomic::Ordering;
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(launch_failed)?;
        #[cfg(windows)]
        let mut job = JobHandle::create();
        #[cfg(windows)]
        if job.as_ref().is_some_and(|job| !job.assign(&child)) {
            job = None;
        }
        let mut out = child.stdout.take().expect("stdout piped");
        let mut err = child.stderr.take().expect("stderr piped");
        let out_t = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = out.read_to_end(&mut b);
            b
        });
        let err_t = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = err.read_to_end(&mut b);
            b
        });
        loop {
            if let Some(status) = child.try_wait()? {
                let stdout = out_t.join().unwrap_or_default();
                let stderr = err_t.join().unwrap_or_default();
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if flag.load(Ordering::Relaxed) {
                #[cfg(windows)]
                kill_tree(&mut child, job.as_ref());
                #[cfg(not(windows))]
                kill_tree(&mut child);
                let _ = child.wait();
                return Err(BackendError::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) {
    let pid = child.id();
    // `kill` is usually a shell builtin; /bin/kill is the procps binary on Linux.
    for prog in ["/bin/kill", "kill"] {
        if std::process::Command::new(prog)
            .args(["-TERM", &format!("-{pid}")])
            .status()
            .is_ok()
        {
            break;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    if child.try_wait().ok().flatten().is_none() {
        for prog in ["/bin/kill", "kill"] {
            if std::process::Command::new(prog)
                .args(["-KILL", &format!("-{pid}")])
                .status()
                .is_ok()
            {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Fallback so we never hang if the group-kill binary is unavailable.
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
}

#[cfg(windows)]
struct JobHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl JobHandle {
    fn create() -> Option<Self> {
        use std::{mem, ptr};
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        let handle = unsafe {
            // SAFETY: Null security attributes and name request a private unnamed job object.
            CreateJobObjectW(ptr::null(), ptr::null())
        };
        if handle.is_null() {
            return None;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe {
            // SAFETY: The Windows limit-information C struct permits zero in every field.
            mem::zeroed()
        };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            // SAFETY: `limits` is initialized and its size matches the selected information class.
            windows_sys::Win32::System::JobObjects::SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &mut limits as *mut _ as *mut _,
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            unsafe {
                // SAFETY: `handle` was returned by CreateJobObjectW above and is owned here.
                windows_sys::Win32::Foundation::CloseHandle(handle);
            }
            return None;
        }
        Some(Self(handle))
    }

    fn assign(&self, child: &std::process::Child) -> bool {
        use std::os::windows::io::AsRawHandle;
        unsafe {
            // SAFETY: The child handle remains owned by `child`; this call only assigns its process.
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.0,
                child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
            ) != 0
        }
    }

    fn terminate(&self) -> bool {
        unsafe {
            // SAFETY: `self.0` is a live job handle owned by this guard.
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1) != 0
        }
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: This guard uniquely owns the valid handle returned by CreateJobObjectW.
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn kill_tree(child: &mut std::process::Child, job: Option<&JobHandle>) {
    if !job.is_some_and(JobHandle::terminate) {
        // Job Object setup can be unavailable in restricted environments; retain legacy behavior.
        let _ = child.kill();
    }
}

struct TempDir {
    path: PathBuf,
}
impl TempDir {
    fn new(kind: &str) -> Result<Self, BackendError> {
        let path = env::temp_dir().join(unique_name(kind));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn readable_model(path: &Path) -> Result<PathBuf, BackendError> {
    let canonical = fs::canonicalize(path).map_err(|e| {
        BackendError::InvalidInput(format!(
            "model file is not readable at {}: {e}",
            path.display()
        ))
    })?;
    if !canonical.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "model path is not a file: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}
/// Validate a qualified Modelica class name (`A.B.C`) before embedding it in a script.
pub fn validate_class_name(class: &str) -> Result<(), BackendError> {
    validate_class(class)
}

/// Escape text for use inside a Modelica string literal in an OMC script.
pub fn escape_modelica_string(s: &str) -> String {
    modelica_string(s)
}

fn validate_class(class: &str) -> Result<(), BackendError> {
    let valid = !class.is_empty()
        && class.split('.').all(|part| {
            let mut chars = part.chars();
            matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    if valid {
        Ok(())
    } else {
        Err(BackendError::InvalidInput(
            "class must be a qualified Modelica name using letters, digits, and underscores".into(),
        ))
    }
}

pub fn validate_parameter_overrides(parameters: &[(String, String)]) -> Result<(), BackendError> {
    let mut seen = std::collections::HashSet::new();
    for (name, value) in parameters {
        validate_class(name).map_err(|_| {
            BackendError::InvalidInput(format!("invalid Modelica parameter name: {name}"))
        })?;
        let number = value.parse::<f64>().map_err(|_| {
            BackendError::InvalidInput(format!(
                "parameter `{name}` must have a finite numeric value"
            ))
        })?;
        if !number.is_finite() {
            return Err(BackendError::InvalidInput(format!(
                "parameter `{name}` must have a finite numeric value"
            )));
        }
        if !seen.insert(name) {
            return Err(BackendError::InvalidInput(format!(
                "parameter override repeated: {name}"
            )));
        }
    }
    Ok(())
}

fn load_libraries_script(libraries: &[(String, String)]) -> Result<String, BackendError> {
    let mut script = String::new();
    for (name, version) in libraries {
        validate_class(name)?;
        if version.is_empty()
            || !version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
        {
            return Err(BackendError::InvalidInput(format!(
                "library {name} must use a non-empty, exact version identifier"
            )));
        }
        script.push_str(&format!(
            "packageLoaded := loadModel({}, {{\"{}\"}});\nif not packageLoaded then print(getErrorString()); end if;\n",
            name,
            modelica_string(version)
        ));
    }
    Ok(script)
}

/// Validate exact dependency versions without needing to start OpenModelica.
pub fn validate_library_pins(libraries: &[(String, String)]) -> Result<(), BackendError> {
    load_libraries_script(libraries).map(|_| ())
}

/// Build the `simulate(...)` statement used by [`Omc::simulate_class`].
///
/// Options that are `None` are left out entirely, so the generated script is byte for byte
/// the one written before `start_time`/`number_of_intervals`/`tolerance`/`method` existed
/// as long as callers do not set them.
fn simulate_call(
    class: &str,
    output_prefix: &str,
    config: &SimulationConfig<'_>,
    simflags: &str,
) -> Result<String, BackendError> {
    let mut call = format!("simulate({}, stopTime={}", class, config.stop_time);
    if let Some(start_time) = config.start_time {
        if !start_time.is_finite() {
            return Err(BackendError::InvalidInput(
                "start time must be finite".into(),
            ));
        }
        if start_time > config.stop_time {
            return Err(BackendError::InvalidInput(format!(
                "start time ({start_time}) must not be later than the stop time ({})",
                config.stop_time
            )));
        }
        call.push_str(&format!(", startTime={start_time}"));
    }
    if let Some(intervals) = config.number_of_intervals {
        if intervals == 0 {
            return Err(BackendError::InvalidInput(
                "number of intervals must be at least 1".into(),
            ));
        }
        call.push_str(&format!(", numberOfIntervals={intervals}"));
    }
    if let Some(tolerance) = config.tolerance {
        if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
            return Err(BackendError::InvalidInput(
                "tolerance must be greater than 0 and smaller than 1".into(),
            ));
        }
        call.push_str(&format!(", tolerance={tolerance}"));
    }
    if let Some(method) = config.method {
        if !SIMULATION_METHODS.contains(&method) {
            return Err(BackendError::InvalidInput(format!(
                "unsupported integration method {method:?}; expected one of {}",
                SIMULATION_METHODS.join(", ")
            )));
        }
        call.push_str(&format!(", method=\"{}\"", modelica_string(method)));
    }
    call.push_str(&format!(
        ", fileNamePrefix=\"{}\", simflags=\"{}\");\nprint(\"OMOLA_RESULT_FILE:\" + simulationResult.resultFile);\n",
        output_prefix,
        modelica_string(simflags)
    ));
    Ok(call)
}

fn absolute_output(path: &Path) -> Result<PathBuf, BackendError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}
fn unique_name(kind: &str) -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("omola-{kind}-{n}-{}", std::process::id())
}
fn has_error_diagnostic(text: &str) -> bool {
    text.contains("Error:")
        || text.contains("Failed to build model:")
        || text.contains("Simulation execution failed for model:")
        || text.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("Error:")
                || trimmed.starts_with("Error ")
                || trimmed.starts_with("Notification: Error")
        })
}
fn contains_false(text: &str) -> bool {
    text.lines()
        .any(|line| matches!(line.trim(), "false" | "\"false\""))
}
fn find_result_file(text: &str) -> Option<PathBuf> {
    for line in text.lines() {
        if let Some((_, value)) = line.split_once("OMOLA_RESULT_FILE:") {
            let value = value.trim().trim_matches('"');
            if !value.is_empty() && value != "<undefined>" {
                return Some(PathBuf::from(value));
            }
            return None;
        }
        if let Some((_, value)) = line.split_once("resultFile =") {
            let value = value.trim().trim_end_matches(',').trim().trim_matches('"');
            if !value.is_empty() && value != "<undefined>" {
                return Some(PathBuf::from(value));
            }
            return None;
        }
    }
    None
}
fn failed(version: &str, out: Output) -> BackendError {
    BackendError::CompilerFailed {
        version: version.into(),
        status: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}
fn output_text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn modelica_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
/// Candidate `omc` locations outside PATH, in priority order: `<OPENMODELICAHOME>/bin`,
/// then `<program_files>/OpenModelica*/bin` sorted by version, newest first.
fn installation_candidates(
    openmodelica_home: Option<PathBuf>,
    program_files: Option<&Path>,
    name: &str,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = openmodelica_home.filter(|h| !h.as_os_str().is_empty()) {
        out.push(home.join("bin").join(name));
    }
    if let Some(root) = program_files {
        let mut dirs: Vec<(Vec<u64>, PathBuf)> = fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let rest = file_name.strip_prefix("OpenModelica")?;
                let version: Vec<u64> = rest
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|p| !p.is_empty())
                    .map(|p| p.parse().unwrap_or(0))
                    .collect();
                Some((version, entry.path().join("bin").join(name)))
            })
            .collect();
        dirs.sort_by(|a, b| b.0.cmp(&a.0));
        out.extend(dirs.into_iter().map(|(_, p)| p));
    }
    out
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn installation_candidates_prefer_home_then_newest_version() {
        let root = env::temp_dir().join(unique_name("pf"));
        for dir in [
            "OpenModelica1.26.3-64bit",
            "OpenModelica1.27.1-64bit",
            "OpenModelica1.9.0",
            "Other",
        ] {
            fs::create_dir_all(root.join(dir).join("bin")).unwrap();
        }
        let found = installation_candidates(Some(PathBuf::from("/om")), Some(&root), "omc.exe");
        assert_eq!(found[0], PathBuf::from("/om").join("bin").join("omc.exe"));
        let names: Vec<String> = found[1..]
            .iter()
            .map(|p| {
                p.parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "OpenModelica1.27.1-64bit",
                "OpenModelica1.26.3-64bit",
                "OpenModelica1.9.0"
            ]
        );
        assert!(installation_candidates(None, None, "omc").is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validates_qualified_names_and_stop_times_before_creating_script() {
        assert!(validate_class("Pkg.Valid_2").is_ok());
        assert!(validate_class("Pkg..Bad").is_err());
        assert!(validate_class("2Bad").is_err());
        assert!(validate_class("Pkg; print(1)").is_err());
        assert!(BackendError::InvalidInput("stop".into())
            .to_string()
            .contains("Invalid request"));
    }

    #[test]
    fn exact_library_pins_reject_injection_and_accept_literal_versions() {
        assert!(validate_library_pins(&[("FixtureLib".into(), "1.0".into())]).is_ok());
        assert!(
            validate_library_pins(&[("FixtureLib".into(), "1.0\") ; print(1)".into())]).is_err()
        );
        assert!(validate_library_pins(&[("Bad;Name".into(), "1.0".into())]).is_err());
    }

    #[test]
    fn parameter_overrides_are_numeric_unique_and_identifier_bounded() {
        assert!(validate_parameter_overrides(&[("gravity".into(), "9.81".into())]).is_ok());
        assert!(validate_parameter_overrides(&[("Demo.gravity".into(), "-1e2".into())]).is_ok());
        assert!(validate_parameter_overrides(&[("gravity".into(), "NaN".into())]).is_err());
        assert!(validate_parameter_overrides(&[("gravity".into(), "1,foo=2".into())]).is_err());
        assert!(validate_parameter_overrides(&[("x;print(1)".into(), "2".into())]).is_err());
        assert!(validate_parameter_overrides(&[
            ("x".into(), "1".into()),
            ("x".into(), "2".into())
        ])
        .is_err());
    }

    #[test]
    fn detects_compiler_failure_text_even_when_process_status_is_success() {
        assert!(has_error_diagnostic("Error: broken model\n"));
        assert!(has_error_diagnostic(
            "messages = \"Failed to build model: Missing\"\n"
        ));
        assert!(!has_error_diagnostic("true\n"));
        assert!(contains_false("false\n"));
        assert_eq!(
            find_result_file("record SimulationResult\n resultFile = \"\"\nend SimulationResult;"),
            None
        );
        assert_eq!(
            find_result_file("OMOLA_RESULT_FILE:run_res.mat\n"),
            Some(PathBuf::from("run_res.mat"))
        );
        assert_eq!(
            find_result_file("record SimulationResult\n resultFile = \"path with spaces/run_res.mat\",\nend SimulationResult;"),
            Some(PathBuf::from("path with spaces/run_res.mat"))
        );
    }

    #[test]
    fn modelica_paths_escape_quotes_and_backslashes() {
        assert_eq!(
            modelica_string(r#"C:\a \"quoted\" folder\m.mo"#),
            r#"C:\\a \\\"quoted\\\" folder\\m.mo"#
        );
    }

    fn simulate_config(
        start_time: Option<f64>,
        number_of_intervals: Option<u32>,
        tolerance: Option<f64>,
        method: Option<&'static str>,
    ) -> SimulationConfig<'static> {
        SimulationConfig {
            stop_time: 2.0,
            start_time,
            number_of_intervals,
            tolerance,
            method,
            result: None,
            libraries: &[],
            package_roots: &[],
            parameters: &[],
        }
    }

    #[test]
    fn simulate_call_is_unchanged_when_no_option_is_set() {
        let call = simulate_call("M", "run", &simulate_config(None, None, None, None), "").unwrap();
        assert_eq!(
            call,
            "simulate(M, stopTime=2, fileNamePrefix=\"run\", simflags=\"\");\nprint(\"OMOLA_RESULT_FILE:\" + simulationResult.resultFile);\n"
        );
    }

    #[test]
    fn simulate_call_passes_every_setting() {
        let call = simulate_call(
            "Modelica.Blocks.Examples.PID_Controller",
            "run",
            &simulate_config(Some(0.0), Some(10), Some(1e-6), Some("euler")),
            "-override=k=2",
        )
        .unwrap();
        assert_eq!(
            call,
            "simulate(Modelica.Blocks.Examples.PID_Controller, stopTime=2, startTime=0, numberOfIntervals=10, tolerance=0.000001, method=\"euler\", fileNamePrefix=\"run\", simflags=\"-override=k=2\");\nprint(\"OMOLA_RESULT_FILE:\" + simulationResult.resultFile);\n"
        );
    }

    #[test]
    fn simulate_call_rejects_unsafe_or_inconsistent_options() {
        let injected = "dassl\"); system(\"x";
        assert!(simulate_call(
            "M",
            "run",
            &simulate_config(None, None, None, Some(injected)),
            ""
        )
        .is_err());
        assert!(simulate_call(
            "M",
            "run",
            &simulate_config(None, None, None, Some("bogus")),
            ""
        )
        .is_err());
        assert!(simulate_call(
            "M",
            "run",
            &simulate_config(Some(3.0), None, None, None),
            ""
        )
        .is_err());
        assert!(simulate_call(
            "M",
            "run",
            &simulate_config(Some(f64::NAN), None, None, None),
            ""
        )
        .is_err());
        assert!(
            simulate_call("M", "run", &simulate_config(None, Some(0), None, None), "").is_err()
        );
        assert!(simulate_call(
            "M",
            "run",
            &simulate_config(None, None, Some(0.0), None),
            ""
        )
        .is_err());
        assert!(simulate_call(
            "M",
            "run",
            &simulate_config(None, None, Some(1.0), None),
            ""
        )
        .is_err());
    }

    #[test]
    fn every_listed_method_is_accepted() {
        for method in SIMULATION_METHODS {
            assert!(
                simulate_call(
                    "M",
                    "run",
                    &simulate_config(None, None, None, Some(*method)),
                    ""
                )
                .is_ok(),
                "{method}"
            );
        }
    }

    #[test]
    fn fake_executable_receives_script_and_is_cleaned_after_success_and_failure() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        struct RestoreEnv(Vec<(&'static str, Option<OsString>)>);
        impl Drop for RestoreEnv {
            fn drop(&mut self) {
                for (key, value) in &self.0 {
                    if let Some(value) = value {
                        env::set_var(key, value);
                    } else {
                        env::remove_var(key);
                    }
                }
            }
        }

        let fixture = TempDir::new("fake-omc").unwrap();
        let source = fixture.path.join("fake_omc.rs");
        let executable = fixture.path.join(if cfg!(windows) {
            "fake omc.exe"
        } else {
            "fake omc"
        });
        let log = fixture.path.join("arguments log.txt");
        fs::write(
            &source,
            r#"
use std::{env, fs, path::PathBuf, process};
fn main() {
    let argument = env::args().nth(1).unwrap_or_default();
    if argument == "--version" { println!("Fake OMC 1.0"); return; }
    fs::write(env::var_os("OMOLA_FAKE_LOG").unwrap(), &argument).unwrap();
    print!("{}", fs::read_to_string(PathBuf::from(&argument)).unwrap());
    match env::var("OMOLA_FAKE_MODE").as_deref() {
        Ok("diagnostic") => print!("Error: injected failure\\n"),
        Ok("exit") => { eprintln!("fake nonzero exit"); process::exit(7); },
        _ => (),
    }
}
"#,
        )
        .unwrap();
        assert!(Command::new("rustc")
            .args([
                "--edition=2021",
                source.to_str().unwrap(),
                "-o",
                executable.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success());

        let keys = ["OMOLA_OMC", "OMOLA_FAKE_LOG", "OMOLA_FAKE_MODE"];
        let _restore = RestoreEnv(keys.iter().map(|key| (*key, env::var_os(key))).collect());
        env::set_var("OMOLA_OMC", &executable);
        env::set_var("OMOLA_FAKE_LOG", &log);
        env::set_var("OMOLA_FAKE_MODE", "diagnostic");
        let omc = Omc::discover().unwrap();
        assert_eq!(omc.executable(), executable);
        let failed_run = omc.run_script("print(1);");
        assert!(
            matches!(&failed_run, Err(BackendError::CompilerDiagnostic { .. })),
            "unexpected fake compiler result: {failed_run:?}"
        );
        let mos = PathBuf::from(fs::read_to_string(&log).unwrap());
        assert!(!mos.exists());
        assert!(!mos.parent().unwrap().exists());

        env::set_var("OMOLA_FAKE_MODE", "success");
        let result = omc.run_script("print(1);").unwrap();
        assert_eq!(result.omc_version, "Fake OMC 1.0");
        assert!(result.output.contains("print(1);"));
        let mos = PathBuf::from(fs::read_to_string(&log).unwrap());
        assert!(!mos.exists());
        assert!(!mos.parent().unwrap().exists());

        env::set_var("OMOLA_FAKE_MODE", "exit");
        let failed_run = omc.run_script("print(2);");
        assert!(matches!(
            &failed_run,
            Err(BackendError::CompilerFailed {
                status: Some(7),
                ..
            })
        ));
        let mos = PathBuf::from(fs::read_to_string(&log).unwrap());
        assert!(!mos.exists());
        assert!(!mos.parent().unwrap().exists());
    }

    #[cfg(unix)]
    #[test]
    fn cancel_flag_stops_long_running_fake_omc_within_two_seconds() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new("cancel").unwrap();
        let fake = dir.path.join("fake-omc");
        fs::write(
            &fake,
            "#!/bin/sh
if [ \"$1\" = \"--version\" ]; then echo 'OpenModelica 1.27.1 (fake)'; exit 0; fi
sleep 30
",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let old = env::var_os("OMOLA_OMC");
        env::set_var("OMOLA_OMC", &fake);
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag_setter = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            flag_setter.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let omc = Omc::discover().unwrap().with_cancel_flag(flag);
        let started = std::time::Instant::now();
        let result = omc.run_script("print(1);");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "took {:?}",
            started.elapsed()
        );
        assert!(
            matches!(result, Err(BackendError::Cancelled)),
            "unexpected: {result:?}"
        );
        // Child sleep processes are killed via the process group (kill -TERM -pid);
        // verifying via pgrep is best-effort and omitted on CI without pgrep, so we only
        // assert the in-time return here. The 2s bound proves the loop does not hang.
        if let Some(old) = old {
            env::set_var("OMOLA_OMC", old);
        } else {
            env::remove_var("OMOLA_OMC");
        }
    }

    #[cfg(windows)]
    #[test]
    fn cancel_flag_terminates_job_object_children_on_windows() {
        struct RestoreOmola(Option<OsString>);
        impl Drop for RestoreOmola {
            fn drop(&mut self) {
                if let Some(value) = &self.0 {
                    env::set_var("OMOLA_OMC", value);
                } else {
                    env::remove_var("OMOLA_OMC");
                }
            }
        }

        let _env_lock = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new("cancel-job-object").unwrap();
        let fake = dir.path.join("fake-omc.bat");
        fs::write(
            &fake,
            "@echo off\r\nif \"%~1\"==\"--version\" (echo OpenModelica 1.27.1 fake & exit /b 0)\r\ncmd /C ping -n 30 127.0.0.1 > NUL\r\n",
        )
        .unwrap();
        let _restore = RestoreOmola(env::var_os("OMOLA_OMC"));
        env::set_var("OMOLA_OMC", &fake);
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag_setter = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            flag_setter.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let omc = Omc::discover().unwrap().with_cancel_flag(flag);
        let started = std::time::Instant::now();
        let result = omc.run_script("print(1);");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "cancellation took {:?}",
            started.elapsed()
        );
        assert!(matches!(result, Err(BackendError::Cancelled)), "{result:?}");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let tasks = Command::new("tasklist")
                .args(["/FI", "IMAGENAME eq PING.EXE", "/FO", "CSV", "/NH"])
                .output()
                .unwrap();
            let listing = String::from_utf8_lossy(&tasks.stdout).to_ascii_uppercase();
            if !listing.contains("PING.EXE") || std::time::Instant::now() >= deadline {
                assert!(
                    !listing.contains("PING.EXE"),
                    "ping.exe remained after cancellation: {listing}"
                );
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
