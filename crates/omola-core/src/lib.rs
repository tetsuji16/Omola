//! Process boundary around an installed OpenModelica Compiler (OMC).
//! Modelica parsing, translation, and simulation remain the external backend's job.

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
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCompiler { searched } => write!(f, "OpenModelica compiler `omc` was not found ({searched}). Install OpenModelica and add its bin directory to PATH, or set OMOLA_OMC to the executable path."),
            Self::InvalidConfiguredCompiler { path } => write!(f, "OMOLA_OMC points to a missing or non-file path: {}", path.display()),
            Self::LaunchFailed { executable, source } => write!(f, "Could not launch OpenModelica at {}: {source}", executable.display()),
            Self::Io(source) => write!(f, "OMC adapter I/O failed: {source}"),
            Self::InvalidInput(message) => write!(f, "Invalid request: {message}"),
            Self::CompilerFailed { version, status, stdout, stderr } => write!(f, "OpenModelica {version} exited with status {status:?}.\nstdout:\n{stdout}\nstderr:\n{stderr}"),
            Self::CompilerDiagnostic { version, output } => write!(f, "OpenModelica {version} reported an error:\n{output}"),
            Self::InvalidCompilerResponse { version, output } => write!(f, "OpenModelica {version} returned no usable result:\n{output}"),
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
}

impl Omc {
    /// Resolve OMOLA_OMC first; when unset/empty, use `omc[.exe]` from PATH.
    pub fn discover() -> Result<Self, BackendError> {
        if let Some(configured) = env::var_os("OMOLA_OMC") {
            if !configured.is_empty() {
                let path = PathBuf::from(configured);
                if !path.is_file() {
                    return Err(BackendError::InvalidConfiguredCompiler { path });
                }
                return Ok(Self { executable: path });
            }
        }
        let name = if cfg!(windows) { "omc.exe" } else { "omc" };
        if let Some(path) = find_on_path(name) {
            return Ok(Self { executable: path });
        }
        Err(BackendError::MissingCompiler {
            searched: "checked OMOLA_OMC and PATH".into(),
        })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
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
        validate_class(class)?;
        let source = readable_model(source)?;
        let script = format!(
            "clearMessages();\nloaded := loadFile(\"{}\");\nif not loaded then print(getErrorString()); end if;\nif loaded then result := checkModel({}); print(result); print(getErrorString()); end if;\n",
            modelica_string(&source.to_string_lossy()), class
        );
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
        validate_class(class)?;
        let source = readable_model(source)?;
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
        let script = format!(
            "clearMessages();\nloaded := loadFile(\"{}\");\nif not loaded then print(getErrorString()); end if;\nsimulationResult := simulate({}, stopTime={}, fileNamePrefix=\"{}\");\nprint(\"OMOLA_RESULT_FILE:\" + simulationResult.resultFile);\n",
            modelica_string(&source.to_string_lossy()), class, stop_time, output_prefix
        );
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

    /// Run a script and return versioned command output. Scratch scripts are deleted on every exit.
    pub fn run_script(&self, script: &str) -> Result<CommandResult, BackendError> {
        self.run_script_in_temp(script).map(|(result, _, _)| result)
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
        command
            .output()
            .map_err(|source| BackendError::LaunchFailed {
                executable: self.executable.clone(),
                source,
            })
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
fn find_on_path(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn fake_executable_receives_script_and_is_cleaned_after_success_and_failure() {
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
}
