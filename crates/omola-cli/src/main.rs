use omola_core::{BackendError, Omc};
use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("doctor") => doctor(),
        Some("check") => check(parse(&args[1..])),
        Some("simulate") => simulate(parse(&args[1..])),
        _ => { usage(); return ExitCode::from(2); }
    };
    match result { Ok(text) => { print!("{text}"); ExitCode::SUCCESS }, Err(e) => { eprintln!("omola: {e}"); ExitCode::FAILURE } }
}
fn doctor() -> Result<String, BackendError> {
    let omc = Omc::discover()?;
    Ok(format!("OpenModelica: {}\nBackend: external `omc` process\n", omc.version()?))
}
fn check(args: Vec<String>) -> Result<String, BackendError> {
    let (model, class, _) = required(&args)?;
    Omc::discover()?.check_model(&model, &class)
}
fn simulate(args: Vec<String>) -> Result<String, BackendError> {
    let (model, class, values) = required(&args)?;
    let stop_time = match values.get("--stop-time") { Some(s) => s.parse::<f64>().map_err(|_| invalid("--stop-time must be a number"))?, None => 1.0 };
    let output = values.get("--output").map(PathBuf::from);
    Omc::discover()?.simulate(&model, &class, stop_time, output.as_deref())
}
fn parse(args: &[String]) -> Vec<String> { args.to_vec() }
fn required(args: &[String]) -> Result<(PathBuf, String, std::collections::HashMap<String,String>), BackendError> {
    let mut values = std::collections::HashMap::new(); let mut i=0;
    while i<args.len() { if args[i].starts_with("--") && i+1<args.len() { values.insert(args[i].clone(),args[i+1].clone()); i+=2; } else { i+=1; } }
    let model = values.get("--model").map(PathBuf::from).ok_or_else(|| invalid("missing --model PATH"))?;
    let class = values.get("--class").cloned().ok_or_else(|| invalid("missing --class Modelica.Class"))?;
    if !model.is_file() { return Err(invalid(&format!("model file does not exist: {}", model.display()))); }
    Ok((model,class,values))
}
fn invalid(message: &str) -> BackendError { BackendError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput,message.to_owned())) }
fn usage() { eprintln!("Omola 0.1 — OpenModelica-backed Modelica workflow\n\nUsage:\n  omola doctor\n  omola check --model FILE.mo --class Modelica.Class\n  omola simulate --model FILE.mo --class Modelica.Class [--stop-time SECONDS] [--output PREFIX]\n\nRequires OpenModelica `omc` on PATH or OMOLA_OMC set to its executable path."); }

