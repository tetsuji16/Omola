use omola_core::{BackendError, Omc};
#[cfg(target_os = "linux")]
use std::process::Stdio;
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) struct VersionStatus {
    pub(crate) display: String,
    pub(crate) warning: Option<String>,
    pub(crate) mismatch: bool,
}

pub(crate) fn discover(configured: Option<&Path>) -> Result<PathBuf, BackendError> {
    if let Some(path) = configured {
        return validate_executable(path, "--omedit");
    }
    if let Some(path) = env::var_os("OMOLA_OMEDIT").filter(|path| !path.is_empty()) {
        return validate_executable(Path::new(&path), "OMOLA_OMEDIT");
    }

    let names: &[&str] = if cfg!(windows) {
        &["OMEdit.exe", "OMEdit"]
    } else {
        &["OMEdit", "omedit"]
    };
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return fs::canonicalize(&candidate).map_err(BackendError::Io);
            }
        }
    }
    Err(BackendError::InvalidInput(
        "OMEdit was not found on PATH; install the OMEdit build matching OpenModelica or set OMOLA_OMEDIT to its executable".into(),
    ))
}

fn validate_executable(path: &Path, source: &str) -> Result<PathBuf, BackendError> {
    if !path.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "{source} does not point to an OMEdit executable file: {}",
            path.display()
        )));
    }
    fs::canonicalize(path).map_err(BackendError::Io)
}

pub(crate) fn verify_version(executable: &Path, omc_version: &str) -> VersionStatus {
    #[cfg(target_os = "linux")]
    {
        match debian_omedit_version(executable) {
            Some(package_version) => {
                let package_release = package_version
                    .split_once('-')
                    .map_or(package_version.as_str(), |(release, _)| release);
                let compiler_release = omc_version
                    .split_whitespace()
                    .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()));
                match compiler_release {
                    Some(release) if release == package_release => VersionStatus {
                        display: format!("OpenModelica package {package_version} (matched)"),
                        warning: None,
                        mismatch: false,
                    },
                    Some(release) => VersionStatus {
                        display: format!("OpenModelica package {package_version} (mismatch)"),
                        warning: Some(format!(
                            "OMEdit package version {package_release} does not match OMC version {release}; install a matching OpenModelica/OMEdit pair"
                        )),
                        mismatch: true,
                    },
                    None => unverified_version(executable),
                }
            }
            None => unverified_version(executable),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = omc_version;
        unverified_version(executable)
    }
}

#[cfg(target_os = "linux")]
fn debian_omedit_version(executable: &Path) -> Option<String> {
    // OMEdit has no documented --version command-line switch. On Debian-based
    // systems, use package metadata only when dpkg confirms ownership of the
    // exact executable selected for launch.
    let owner = Command::new("dpkg-query")
        .args([OsStr::new("-S")])
        .arg(executable)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !owner.status.success() {
        return None;
    }
    let owner = String::from_utf8_lossy(&owner.stdout);
    let (package, _) = owner.trim().split_once(':')?;
    if package.trim() != "omedit" {
        return None;
    }

    let version = Command::new("dpkg-query")
        .args([
            OsStr::new("-W"),
            OsStr::new("-f=${Version}"),
            OsStr::new("omedit"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    version
        .status
        .success()
        .then(|| String::from_utf8_lossy(&version.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn unverified_version(executable: &Path) -> VersionStatus {
    VersionStatus {
        display: format!("{} (unverified)", executable.display()),
        warning: Some(format!(
            "could not verify the selected OMEdit version at {}; confirm that it matches the OMC version before using this project",
            executable.display()
        )),
        mismatch: false,
    }
}

pub(crate) fn project_library_path(
    omc: &Omc,
    package_roots: &[PathBuf],
) -> Result<OsString, BackendError> {
    let result = omc.run_script("print(\"OMOLA_MODELICAPATH:\" + getModelicaPath());\n")?;
    let current_path = result
        .output
        .lines()
        .find_map(|line| line.strip_prefix("OMOLA_MODELICAPATH:"))
        .ok_or_else(|| BackendError::InvalidCompilerResponse {
            version: result.omc_version.clone(),
            output: result.output.clone(),
        })?;
    prepend_package_roots(package_roots, OsStr::new(current_path))
}

fn prepend_package_roots(
    package_roots: &[PathBuf],
    current_path: &OsStr,
) -> Result<OsString, BackendError> {
    let mut paths = package_roots.to_vec();
    paths.extend(env::split_paths(current_path));
    env::join_paths(paths).map_err(|error| {
        BackendError::InvalidInput(format!(
            "cannot combine project library roots with OpenModelica's current library path: {error}"
        ))
    })
}

pub(crate) fn launch(
    executable: &Path,
    model_file: &Path,
    project_root: &Path,
    modelica_path: &OsStr,
) -> Result<i32, BackendError> {
    let status = Command::new(executable)
        .arg(model_file)
        .current_dir(project_root)
        .env("OPENMODELICALIBRARY", modelica_path)
        .status()
        .map_err(|source| BackendError::LaunchFailed {
            executable: executable.to_path_buf(),
            source,
        })?;
    Ok(status.code().unwrap_or(128))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_library_path_composition_prepends_roots_and_preserves_installation_path() {
        let root = PathBuf::from("project libs/Fixture 1.0");
        let existing = env::join_paths(["system libraries", "user libraries"]).unwrap();
        let joined = prepend_package_roots(std::slice::from_ref(&root), &existing).unwrap();
        let paths: Vec<_> = env::split_paths(&joined).collect();
        assert_eq!(
            paths,
            [
                root,
                PathBuf::from("system libraries"),
                PathBuf::from("user libraries")
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn launch_preserves_argument_spaces_does_not_change_source_and_waits_for_exit() {
        use sha2::{Digest, Sha256};
        use std::{os::unix::fs::PermissionsExt, time::SystemTime};

        let temp = env::temp_dir().join(format!(
            "omola omedit test {}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let model = temp.join("Bouncing Ball.mo");
        let executable = temp.join("fake omedit");
        fs::write(&model, "model BouncingBall end BouncingBall;\n").unwrap();
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$1.captured\"\nprintf '%s\\n' \"$OPENMODELICALIBRARY\" >> \"$1.captured\"\nexit 0\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        let before = Sha256::digest(fs::read(&model).unwrap());
        let exit_code = launch(
            &executable,
            &model,
            &temp,
            OsStr::new("root path;with spaces"),
        )
        .unwrap();
        assert_eq!(exit_code, 0);
        let captured = fs::read_to_string(format!("{}.captured", model.display())).unwrap();
        let mut lines = captured.lines();
        assert_eq!(lines.next().unwrap(), model.to_string_lossy());
        assert_eq!(lines.next().unwrap(), "root path;with spaces");
        assert!(lines.next().is_none());
        assert_eq!(before, Sha256::digest(fs::read(&model).unwrap()));
        fs::remove_dir_all(temp).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn launch_returns_nonzero_child_status() {
        use std::{os::unix::fs::PermissionsExt, time::SystemTime};

        let temp = env::temp_dir().join(format!(
            "omola omedit exit test {}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let executable = temp.join("fake omedit");
        fs::write(&executable, "#!/bin/sh\nexit 7\n").unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let code = launch(&executable, Path::new("model.mo"), &temp, OsStr::new("")).unwrap();
        assert_eq!(code, 7);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn unsupported_host_version_probe_is_diagnostic_not_a_hard_failure() {
        let status = unverified_version(Path::new("OMEdit"));
        assert!(status.warning.unwrap().contains("confirm that it matches"));
    }
}
