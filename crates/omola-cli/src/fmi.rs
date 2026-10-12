use omola_core::{BackendError, Omc};
use quick_xml::{events::Event, Reader};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

#[derive(Serialize)]
struct FmuMetadata {
    schema_version: u32,
    fmi_version: String,
    fmu_type: String,
    model: String,
    class: String,
    omc_version: String,
    platform: String,
    build_options: Vec<String>,
    exported_at_unix_ms: u128,
}

#[derive(Serialize)]
struct ImportMetadata {
    schema_version: u32,
    fmi_version: String,
    source_fmu: String,
    generated_model: String,
    omc_version: String,
    limitation: String,
    imported_at_unix_ms: u128,
}

pub fn run(args: &[String]) -> Result<String, BackendError> {
    match args.first().map(String::as_str) {
        Some("capabilities") if args.len() == 1 => capabilities(),
        Some("export") => export(&args[1..]),
        Some("import") => import(&args[1..]),
        Some("validate") if args.len() == 2 => validate(Path::new(&args[1])),
        _ => Err(usage()),
    }
}

fn capabilities() -> Result<String, BackendError> {
    let omc = Omc::discover()?;
    let version = omc.version()?;
    let profile = capability_profile(&version);
    Ok(format!(
        "OMC: {version}\nCapability source: OpenModelica User's Guide 1.27.1 ({profile})\nExport (documented): FMI 1.0 Model Exchange; FMI 2.0.4 Model Exchange and Co-Simulation (OMC selector: 2.0)\nImport (documented): FMI 2.0 Model Exchange wrapper into Modelica; experimental and subject to non-standard Modelica flag\nLive capability probing: unavailable\nDymola parity: not implied by FMI support\n"
    ))
}

fn capability_profile(version: &str) -> &'static str {
    if version.contains("1.27.1") {
        "matches the pinned 1.27.1 user-guide profile"
    } else {
        "differs from the pinned 1.27.1 documentation profile; live FMI probing is unavailable"
    }
}

fn import(args: &[String]) -> Result<String, BackendError> {
    let mut file = None;
    let mut output_dir = None;
    let mut index = 0;
    while index < args.len() {
        let key = &args[index];
        let value = args.get(index + 1).ok_or_else(usage)?;
        let slot = match key.as_str() {
            "--file" => &mut file,
            "--output-dir" => &mut output_dir,
            _ => return Err(usage()),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(BackendError::InvalidInput(format!(
                "duplicate FMI import option {key}"
            )));
        }
        index += 2;
    }
    let source = PathBuf::from(file.ok_or_else(usage)?);
    let output_dir = PathBuf::from(output_dir.ok_or_else(usage)?);
    let report = validate_fmu(&source)?;
    if report.fmi_version != "2.0" || !report.interfaces.iter().any(|i| i == "Model Exchange") {
        return Err(BackendError::InvalidInput(
            "OMC 1.27.1 import path is documented only for FMI 2.0 Model Exchange FMUs".into(),
        ));
    }
    let omc = Omc::discover()?;
    let result = omc.import_fmu(&source, &output_dir)?;
    let wrapper = result
        .result_file
        .as_ref()
        .expect("import sets generated model");
    let metadata_path = output_dir.join("omola-import.json");
    let metadata = ImportMetadata {
        schema_version: 1,
        fmi_version: report.fmi_version.clone(),
        source_fmu: source
            .canonicalize()
            .unwrap_or(source)
            .display()
            .to_string(),
        generated_model: wrapper.display().to_string(),
        omc_version: result.omc_version.clone(),
        limitation: "OMC FMI import is experimental and wraps FMI 2.0 Model Exchange as non-standard Modelica; no Omola FMI runtime or behavioral-equivalence check is performed".into(),
        imported_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    };
    let bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| {
        BackendError::InvalidInput(format!("invalid FMI import metadata: {error}"))
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| {
            BackendError::InvalidInput(format!(
                "could not create FMI import metadata {}: {error}",
                metadata_path.display()
            ))
        })?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    Ok(format!(
        "Imported FMI {} Model Exchange to {}\nGenerated Modelica wrapper: {}\nMetadata: {}\nLimitation: {}\n",
        report.fmi_version,
        output_dir.display(),
        wrapper.display(),
        metadata_path.display(),
        metadata.limitation
    ))
}

fn export(args: &[String]) -> Result<String, BackendError> {
    let mut model = None;
    let mut class = None;
    let mut version = None;
    let mut fmu_type = None;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        let key = &args[index];
        let value = args.get(index + 1).ok_or_else(usage)?;
        let slot = match key.as_str() {
            "--model" => &mut model,
            "--class" => &mut class,
            "--version" => &mut version,
            "--type" => &mut fmu_type,
            "--output" => &mut output,
            _ => return Err(usage()),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(BackendError::InvalidInput(format!(
                "duplicate FMI export option {key}"
            )));
        }
        index += 2;
    }
    let model = model.ok_or_else(usage)?;
    let class = class.ok_or_else(usage)?;
    let version = version.ok_or_else(usage)?;
    let fmu_type = fmu_type.ok_or_else(usage)?;
    let output = PathBuf::from(output.ok_or_else(usage)?);
    let source = PathBuf::from(model);
    let omc = Omc::discover()?;
    let result = omc.export_fmu(&source, &class, &version, &fmu_type, &output)?;
    let report = validate_fmu(&output)?;
    let metadata_path = output.with_extension("json");
    let metadata = FmuMetadata {
        schema_version: 1,
        fmi_version: report.fmi_version.clone(),
        fmu_type: fmu_type.clone(),
        model: fs::canonicalize(&source)
            .unwrap_or(source)
            .display()
            .to_string(),
        class,
        omc_version: result.omc_version.clone(),
        platform: std::env::consts::ARCH.to_owned() + "-" + std::env::consts::OS,
        build_options: vec![format!("version={version}"), format!("fmuType={fmu_type}")],
        exported_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    };
    let sidecar = serde_json::to_vec_pretty(&metadata)
        .map_err(|error| BackendError::InvalidInput(format!("invalid FMU metadata: {error}")))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| {
            BackendError::InvalidInput(format!(
                "could not create FMU metadata {} without overwriting: {error}",
                metadata_path.display()
            ))
        })?;
    file.write_all(&sidecar)?;
    file.write_all(b"\n")?;
    Ok(format!(
        "Exported {} ({}) with {}\nValidated FMU: {}\nMetadata: {}\n",
        report.model_name,
        report.fmi_version,
        report.interfaces.join(", "),
        output.display(),
        metadata_path.display()
    ))
}

#[derive(Debug)]
struct FmuReport {
    fmi_version: String,
    model_name: String,
    interfaces: Vec<String>,
}

fn validate(path: &Path) -> Result<String, BackendError> {
    if !path.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "FMU archive does not exist or is not a file: {}",
            path.display()
        )));
    }
    let report = validate_fmu(path)?;
    Ok(format!(
        "Valid FMU archive: {}\nFMI version: {}\nModel name: {}\nInterfaces: {}\n",
        path.display(),
        report.fmi_version,
        report.model_name,
        report.interfaces.join(", ")
    ))
}

fn validate_fmu(path: &Path) -> Result<FmuReport, BackendError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        BackendError::InvalidInput(format!("not a readable FMU ZIP archive: {error}"))
    })?;
    let has_binary = archive.file_names().any(|name| {
        name.strip_prefix("binaries/")
            .is_some_and(|relative| relative.contains('/') && !name.ends_with('/'))
    });
    let mut description = archive.by_name("modelDescription.xml").map_err(|error| {
        BackendError::InvalidInput(format!(
            "FMU archive is missing modelDescription.xml: {error}"
        ))
    })?;
    let mut xml = String::new();
    description.read_to_string(&mut xml).map_err(|error| {
        BackendError::InvalidInput(format!("cannot read modelDescription.xml: {error}"))
    })?;
    drop(description);
    if !has_binary {
        return Err(BackendError::InvalidInput(
            "FMU archive has no binaries/ entry; this validator requires a platform binary".into(),
        ));
    }
    parse_model_description(&xml)
}

fn parse_model_description(xml: &str) -> Result<FmuReport, BackendError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut fmi_version = None;
    let mut model_name = None;
    let mut has_root = false;
    let mut saw_element = false;
    let mut interfaces = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let tag = element.local_name();
                if !saw_element {
                    saw_element = true;
                    if tag.as_ref() != b"fmiModelDescription" {
                        return Err(BackendError::InvalidInput(
                            "modelDescription.xml root element must be fmiModelDescription".into(),
                        ));
                    }
                }
                if tag.as_ref() == b"fmiModelDescription" {
                    has_root = true;
                    for attribute in element.attributes().with_checks(true) {
                        let attribute = attribute.map_err(|error| {
                            BackendError::InvalidInput(format!(
                                "invalid modelDescription.xml attribute: {error}"
                            ))
                        })?;
                        let value = attribute
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(|error| {
                                BackendError::InvalidInput(format!(
                                    "invalid modelDescription.xml value: {error}"
                                ))
                            })?
                            .into_owned();
                        match attribute.key.local_name().as_ref() {
                            b"fmiVersion" => fmi_version = Some(value),
                            b"modelName" => model_name = Some(value),
                            _ => {}
                        }
                    }
                } else if tag.as_ref() == b"ModelExchange" {
                    interfaces.push("Model Exchange".to_owned());
                } else if tag.as_ref() == b"CoSimulation" {
                    interfaces.push("Co-Simulation".to_owned());
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(BackendError::InvalidInput(format!(
                    "invalid modelDescription.xml: {error}"
                )))
            }
            _ => {}
        }
    }
    if !has_root {
        return Err(BackendError::InvalidInput(
            "modelDescription.xml has no fmiModelDescription root element".into(),
        ));
    }
    let fmi_version = fmi_version.ok_or_else(|| {
        BackendError::InvalidInput("modelDescription.xml has no fmiVersion".into())
    })?;
    let model_name = model_name.ok_or_else(|| {
        BackendError::InvalidInput("modelDescription.xml has no modelName".into())
    })?;
    if interfaces.is_empty() {
        return Err(BackendError::InvalidInput(
            "modelDescription.xml declares no FMI interface".into(),
        ));
    }
    Ok(FmuReport {
        fmi_version,
        model_name,
        interfaces,
    })
}

fn usage() -> BackendError {
    BackendError::InvalidInput(
        "usage: fmi capabilities | fmi export --model FILE.mo --class Model.Name --version 2.0 --type me|cs|me_cs --output FILE.fmu | fmi validate FILE.fmu | fmi import --file FILE.fmu --output-dir NEW_DIRECTORY".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_description_requires_valid_fmi_metadata_and_interface() {
        let xml = r#"<fmiModelDescription fmiVersion="2.0" modelName="Example"><CoSimulation/></fmiModelDescription>"#;
        let result = parse_model_description(xml).unwrap();
        assert_eq!(result.fmi_version, "2.0");
        assert_eq!(result.model_name, "Example");
        assert_eq!(result.interfaces, ["Co-Simulation"]);
        assert!(parse_model_description("<fmiModelDescription/>").is_err());
        assert!(parse_model_description("<wrapper><fmiModelDescription fmiVersion=\"2.0\" modelName=\"Example\"><CoSimulation/></fmiModelDescription></wrapper>").is_err());
    }

    #[test]
    fn unsupported_interface_is_not_accepted() {
        let xml = r#"<fmiModelDescription fmiVersion="2.0" modelName="Example"/>"#;
        assert!(parse_model_description(xml)
            .unwrap_err()
            .to_string()
            .contains("no FMI interface"));
    }

    #[test]
    fn export_options_reject_unknown_and_duplicate_arguments_without_omc() {
        let unknown = run(&["export".into(), "--unsupported".into(), "x".into()]);
        assert!(unknown.unwrap_err().to_string().contains("usage: fmi"));
        let duplicate = run(&[
            "export".into(),
            "--model".into(),
            "a.mo".into(),
            "--model".into(),
            "b.mo".into(),
        ]);
        assert!(duplicate
            .unwrap_err()
            .to_string()
            .contains("duplicate FMI export option"));
    }

    #[test]
    fn capability_profile_is_explicit_for_unprobed_versions() {
        assert!(capability_profile("OpenModelica 1.27.1").contains("matches"));
        assert!(
            capability_profile("OpenModelica 1.28.0").contains("live FMI probing is unavailable")
        );
    }

    #[test]
    fn missing_omc_reports_a_capability_probe_setup_diagnostic() {
        let old = std::env::var_os("OMOLA_OMC");
        std::env::set_var("OMOLA_OMC", "target/omc-that-does-not-exist");
        let error = capabilities().unwrap_err().to_string();
        if let Some(old) = old {
            std::env::set_var("OMOLA_OMC", old);
        } else {
            std::env::remove_var("OMOLA_OMC");
        }
        assert!(error.contains("OMOLA_OMC points to a missing or non-file path"));
    }

    #[test]
    fn import_arguments_are_validated_before_omc_is_required() {
        let error = run(&["import".into(), "--file".into()]).unwrap_err();
        assert!(error.to_string().contains("usage: fmi"));
    }
}
