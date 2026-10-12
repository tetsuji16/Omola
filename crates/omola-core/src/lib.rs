//! Small process boundary around the installed OpenModelica Compiler (OMC).
//! Omola owns orchestration and diagnostics; OMC owns Modelica translation/simulation.
use std::{env, ffi::OsString, fs, io, path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};

#[derive(Debug)]
pub enum BackendError {
    MissingCompiler { searched: String },
    Io(io::Error),
    CompilerFailed { status: Option<i32>, stdout: String, stderr: String },
}
impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCompiler { searched } => write!(f, "OpenModelica compiler `omc` was not found. {searched}. Install OpenModelica and add its bin directory to PATH, or set OMOLA_OMC to the full compiler path."),
            Self::Io(e) => write!(f, "Could not start or use OpenModelica: {e}"),
            Self::CompilerFailed { status, stdout, stderr } => write!(f, "OpenModelica exited with status {status:?}.\nstdout:\n{stdout}\nstderr:\n{stderr}"),
        }
    }
}
impl std::error::Error for BackendError {}
impl From<io::Error> for BackendError { fn from(value: io::Error) -> Self { Self::Io(value) } }

#[derive(Debug, Clone)]
pub struct Omc { executable: OsString }
impl Omc {
    pub fn discover() -> Result<Self, BackendError> {
        if let Some(path) = env::var_os("OMOLA_OMC") {
            if !path.is_empty() {
                if !Path::new(&path).is_file() { return Err(BackendError::Io(io::Error::new(io::ErrorKind::NotFound, format!("OMOLA_OMC path not found: {}", PathBuf::from(&path).display())))); }
                return Ok(Self { executable: path });
            }
        }
        if let Some(path) = find_on_path(if cfg!(windows) { "omc.exe" } else { "omc" }) {
            return Ok(Self { executable: path.into_os_string() });
        }
        Err(BackendError::MissingCompiler { searched: "checked OMOLA_OMC and PATH".into() })
    }
    pub fn version(&self) -> Result<String, BackendError> {
        let out = Command::new(&self.executable).arg("--version").output()?;
        let text = output_text(&out);
        if !out.status.success() { return Err(failed(out)); }
        Ok(text.trim().to_owned())
    }
    pub fn run_script(&self, script: &str) -> Result<String, BackendError> {
        let path = temp_script_path();
        fs::write(&path, script)?;
        let result = Command::new(&self.executable).arg(&path).output();
        let _ = fs::remove_file(&path);
        let out = result?;
        let text = output_text(&out);
        if !out.status.success() || has_backend_failure(&text) { return Err(failed(out)); }
        Ok(text)
    }
    pub fn check_model(&self, source: &Path, class: &str) -> Result<String, BackendError> {
        validate_class(class)?;
        let source = fs::canonicalize(source)?;
        let script = format!("clearMessages();\nloadFile(\"{}\");\ngetErrorString();\ncheckModel({});\ngetErrorString();\n", modelica_string(&source.to_string_lossy()), class);
        self.run_script(&script)
    }
    pub fn simulate(&self, source: &Path, class: &str, stop_time: f64, result: Option<&Path>) -> Result<String, BackendError> {
        validate_class(class)?;
        let source = fs::canonicalize(source)?;
        if !stop_time.is_finite() || stop_time < 0.0 { return Err(invalid("stop time must be finite and non-negative")); }
        let prefix = match result { Some(path) => absolute_output(path)?, None => temp_script_path().with_extension("") };
        let script = format!("clearMessages();\nloadFile(\"{}\");\ngetErrorString();\nsimulate({}, stopTime={}, fileNamePrefix=\"{}\");\ngetErrorString();\n", modelica_string(&source.to_string_lossy()), class, stop_time, modelica_string(&prefix.to_string_lossy()));
        self.run_script(&script)
    }
}
fn invalid(message: &str) -> BackendError { BackendError::Io(io::Error::new(io::ErrorKind::InvalidInput, message)) }
fn validate_class(class: &str) -> Result<(), BackendError> {
    let valid = !class.is_empty() && class.split('.').all(|part| {
        let mut chars = part.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    if valid { Ok(()) } else { Err(invalid("class must be a qualified Modelica name using letters, digits, and underscores")) }
}
fn absolute_output(path: &Path) -> Result<PathBuf, BackendError> {
    if path.is_absolute() { Ok(path.to_path_buf()) } else { Ok(env::current_dir()?.join(path)) }
}
fn has_backend_failure(text: &str) -> bool {
    text.contains("Error:") || text.lines().any(|line| {
        let line = line.trim();
        line == "false" || line == "\"false\"" || line.contains("resultFile = \"\"")
    })
}
fn failed(out: Output) -> BackendError { BackendError::CompilerFailed { status: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() } }
fn output_text(out: &Output) -> String { format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)) }
fn modelica_string(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"") }
fn temp_script_path() -> PathBuf {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    env::temp_dir().join(format!("omola-{n}-{}.mos", std::process::id()))
}
fn find_on_path(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?).map(|p| p.join(name)).find(|p| p.is_file())
}


