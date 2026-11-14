//! `omola mcp` — a Model Context Protocol server over stdio.
//!
//! The server speaks newline-delimited JSON-RPC 2.0 on stdin/stdout, as the MCP
//! stdio binding requires, and never writes anything else to stdout. It supports
//! both protocol eras:
//!
//! * **Modern** (`2026-07-28`): stateless. Every request carries its version and
//!   capabilities in `_meta`, and `server/discover` reports what we support.
//! * **Legacy** (`2025-11-25` and earlier): the `initialize` handshake.
//!
//! The tools are thin wrappers over the same code paths as the CLI, so an agent
//! gets exactly what `omola check` / `omola simulate` / `omola results` print.

use omola_core::BackendError;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

const SERVER_NAME: &str = "omola";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The revision that carries protocol metadata per request.
const MODERN_VERSION: &str = "2026-07-28";

/// Revisions that establish a session with `initialize`, newest first. The
/// server answers a legacy `initialize` with the newest revision it knows when
/// the client asks for something it does not support.
const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Omola runs OpenModelica (`omc`). Call omola_doctor first to confirm the compiler, \
omola_check to validate a class before simulating, omola_simulate to produce a `.mat` result, and \
omola_inspect_result or omola_export_csv to read that result. Paths are resolved on the machine that \
runs the server; nothing is downloaded and no network access is performed.";

/// JSON-RPC error codes. `-32020..=-32099` belong to MCP; the rest are JSON-RPC.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// Arguments each tool accepts, in tool order. This mirrors the
/// `additionalProperties: false` promise in the published schemas: an unknown
/// key is a client bug, so it is rejected as a protocol error rather than being
/// silently ignored.
const TOOL_ARGUMENTS: &[(&str, &[&str])] = &[
    ("omola_doctor", &[]),
    ("omola_check", &["model", "class"]),
    (
        "omola_simulate",
        &["model", "class", "stop_time", "parameters", "output"],
    ),
    ("omola_inspect_result", &["file"]),
    ("omola_export_csv", &["file", "variables", "output"]),
];

fn tool_arguments(name: &str) -> Option<&'static [&'static str]> {
    TOOL_ARGUMENTS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|(_, arguments)| *arguments)
}

pub fn run(args: &[String]) -> Result<String, BackendError> {
    match args {
        [] => serve(),
        [flag] if flag == "--help" || flag == "-h" => Ok(usage()),
        _ => Err(BackendError::InvalidInput(
            "mcp takes no arguments; it serves the Model Context Protocol on stdin/stdout".into(),
        )),
    }
}

fn usage() -> String {
    "Omola MCP server (stdio)\n\nUsage:\n  omola mcp\n\nReads newline-delimited JSON-RPC 2.0 from stdin and writes replies to stdout.\nLogs go to stderr only. The server exits when stdin is closed.\n\nTools: omola_doctor, omola_check, omola_simulate, omola_inspect_result, omola_export_csv\nSee docs/reference/mcp.md.\n"
        .into()
}

fn serve() -> Result<String, BackendError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut server = Server;
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(line) {
            let encoded = serde_json::to_string(&response).map_err(|error| {
                BackendError::InvalidInput(format!("could not encode MCP response: {error}"))
            })?;
            // One message per line; the binding forbids embedded newlines.
            writeln!(stdout, "{encoded}")?;
            stdout.flush()?;
        }
    }
    Ok(String::new())
}

/// The server keeps no session state: `2026-07-28` forbids it, and legacy clients
/// only need the handshake to succeed.
struct Server;

struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    fn with_data(code: i64, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

impl Server {
    /// Handle one line of stdin. Returns the reply to write, or `None` for
    /// notifications and for input that cannot be answered.
    fn handle_line(&mut self, line: &str) -> Option<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(error_response(
                    Value::Null,
                    &RpcError::new(PARSE_ERROR, format!("Parse error: {error}")),
                ))
            }
        };
        let Some(object) = message.as_object() else {
            return Some(error_response(
                Value::Null,
                &RpcError::new(INVALID_REQUEST, "Invalid Request: expected a JSON object"),
            ));
        };
        // A missing id marks a notification, which must not be answered.
        let id = object.get("id").cloned();
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return id.map(|id| {
                error_response(
                    id,
                    &RpcError::new(INVALID_REQUEST, "Invalid Request: missing method"),
                )
            });
        };
        let params = object.get("params").cloned().unwrap_or(Value::Null);

        if id.is_none() {
            self.handle_notification(method, &params);
            return None;
        }
        let id = id.unwrap_or(Value::Null);

        // `_meta` decides the era. Legacy clients send no `_meta` at all.
        let modern = params
            .get("_meta")
            .and_then(|meta| meta.get("io.modelcontextprotocol/protocolVersion"))
            .and_then(Value::as_str);

        let result = match modern {
            Some(version) => self.handle_modern(version, method, &params),
            None => self.handle_legacy(method, &params),
        };

        Some(match result {
            Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
            Err(error) => error_response(id, &error),
        })
    }

    fn handle_notification(&mut self, method: &str, params: &Value) {
        if method == "notifications/initialized" {
            return;
        }
        // `notifications/cancelled` is accepted and ignored: every tool call is
        // synchronous and finishes in bounded time, so there is nothing to stop.
        if std::env::var_os("OMOLA_MCP_LOG").is_some() {
            eprintln!("omola mcp: notification {method} {params}");
        }
    }

    fn handle_modern(
        &mut self,
        version: &str,
        method: &str,
        params: &Value,
    ) -> Result<Value, RpcError> {
        if !is_supported(version) {
            return Err(RpcError::with_data(
                UNSUPPORTED_PROTOCOL_VERSION,
                "Unsupported protocol version",
                json!({"supported": supported_versions(), "requested": version}),
            ));
        }
        // Required by 2026-07-28; a request without it is malformed.
        let meta = params.get("_meta").cloned().unwrap_or(Value::Null);
        if meta
            .get("io.modelcontextprotocol/clientCapabilities")
            .is_none()
        {
            return Err(RpcError::new(
                INVALID_PARAMS,
                "Invalid params: _meta.io.modelcontextprotocol/clientCapabilities is required",
            ));
        }
        let result = match method {
            "server/discover" => json!({
                "resultType": "complete",
                "supportedVersions": supported_versions(),
                "capabilities": capabilities(),
                "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
                "instructions": INSTRUCTIONS,
            }),
            "tools/list" => json!({"resultType": "complete", "tools": tools()}),
            "tools/call" => modern_tool_call(params)?,
            "ping" => json!({"resultType": "complete"}),
            other => {
                return Err(RpcError::new(
                    METHOD_NOT_FOUND,
                    format!("Method not found: {other}"),
                ))
            }
        };
        Ok(result)
    }

    fn handle_legacy(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = if is_supported(requested) {
                    requested
                } else {
                    LEGACY_VERSIONS[0]
                };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": capabilities(),
                    "serverInfo": server_info(),
                    "instructions": INSTRUCTIONS,
                }))
            }
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => legacy_tool_call(params),
            "ping" => Ok(json!({})),
            other => Err(RpcError::new(
                METHOD_NOT_FOUND,
                format!("Method not found: {other}"),
            )),
        }
    }
}

fn supported_versions() -> Vec<&'static str> {
    let mut versions = vec![MODERN_VERSION];
    versions.extend_from_slice(LEGACY_VERSIONS);
    versions
}

fn is_supported(version: &str) -> bool {
    version == MODERN_VERSION || LEGACY_VERSIONS.contains(&version)
}

fn capabilities() -> Value {
    // The tool set is fixed, so `listChanged` is absent (nothing to notify).
    json!({"tools": {}})
}

fn server_info() -> Value {
    json!({"name": SERVER_NAME, "version": SERVER_VERSION})
}

fn error_response(id: Value, error: &RpcError) -> Value {
    let mut body = json!({"code": error.code, "message": error.message});
    if let Some(data) = &error.data {
        body["data"] = data.clone();
    }
    json!({"jsonrpc": "2.0", "id": id, "error": body})
}

fn modern_tool_call(params: &Value) -> Result<Value, RpcError> {
    let mut result = dispatch_tool(params)?;
    result["resultType"] = json!("complete");
    Ok(result)
}

fn legacy_tool_call(params: &Value) -> Result<Value, RpcError> {
    dispatch_tool(params)
}

/// Run a tool and shape the result as MCP content. Tool failures are reported in
/// the result with `isError: true` so the model can read the diagnostic and
/// retry; only protocol-level mistakes (unknown tool, missing name) are JSON-RPC
/// errors.
fn dispatch_tool(params: &Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "Invalid params: `name` is required"))?;
    let Some(allowed) = tool_arguments(name) else {
        return Err(RpcError::new(
            INVALID_PARAMS,
            format!("Unknown tool: {name}"),
        ));
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let Some(object) = arguments.as_object() else {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "Invalid params: `arguments` must be an object",
        ));
    };
    if let Some(unknown) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(RpcError::new(
            INVALID_PARAMS,
            format!("Invalid params: `{name}` does not take `{unknown}`"),
        ));
    }
    Ok(match call_tool(name, &arguments) {
        Ok(text) => json!({
            "content": [{"type": "text", "text": text}],
            "isError": false,
        }),
        Err(message) => json!({
            "content": [{"type": "text", "text": message}],
            "isError": true,
        }),
    })
}

fn call_tool(name: &str, arguments: &Value) -> Result<String, String> {
    match name {
        "omola_doctor" => Ok(crate::doctor().map_err(|error| error.to_string())?),
        "omola_check" => check(arguments),
        "omola_simulate" => simulate(arguments),
        "omola_inspect_result" => inspect_result(arguments),
        "omola_export_csv" => export_csv(arguments),
        other => Err(format!("unknown tool `{other}`")),
    }
}

fn check(arguments: &Value) -> Result<String, String> {
    let model = required_string(arguments, "model")?;
    let class = required_string(arguments, "class")?;
    crate::check(&[
        "--model".into(),
        model.into(),
        "--class".into(),
        class.into(),
    ])
    .map_err(|error| error.to_string())
}

fn simulate(arguments: &Value) -> Result<String, String> {
    let model = required_string(arguments, "model")?;
    let class = required_string(arguments, "class")?;
    let mut cli = vec![
        "--model".to_string(),
        model.to_string(),
        "--class".to_string(),
        class.to_string(),
    ];
    if let Some(stop_time) = arguments.get("stop_time") {
        let value = stop_time
            .as_f64()
            .ok_or_else(|| "`stop_time` must be a number of seconds".to_string())?;
        cli.push("--stop-time".into());
        cli.push(value.to_string());
    }
    if let Some(parameters) = arguments.get("parameters") {
        let map = parameters
            .as_object()
            .ok_or_else(|| "`parameters` must be an object of name to numeric value".to_string())?;
        if !map.is_empty() {
            // Sorted so the same request always produces the same OMC script.
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(name, _)| name.as_str());
            let mut pairs = Vec::new();
            for (name, value) in entries {
                let number = match value {
                    Value::String(text) => text.clone(),
                    Value::Number(number) => number.to_string(),
                    _ => {
                        return Err(format!(
                            "`parameters.{name}` must be a number or a numeric string"
                        ))
                    }
                };
                pairs.push((name.clone(), number));
            }
            // Validate the pairs as given before they are serialised: a value
            // that contains `,` or `=` would otherwise split into a second
            // override once joined.
            omola_core::validate_parameter_overrides(&pairs).map_err(|error| error.to_string())?;
            cli.push("--parameters".into());
            cli.push(
                pairs
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
    }
    if let Some(output) = arguments.get("output") {
        let output = output
            .as_str()
            .ok_or_else(|| "`output` must be a path prefix string".to_string())?;
        cli.push("--output".into());
        cli.push(output.into());
    }
    crate::simulate(&cli).map_err(|error| error.to_string())
}

fn inspect_result(arguments: &Value) -> Result<String, String> {
    let file = required_string(arguments, "file")?;
    crate::results::inspect(std::path::Path::new(file)).map_err(|error| error.to_string())
}

fn export_csv(arguments: &Value) -> Result<String, String> {
    let file = required_string(arguments, "file")?;
    let output = required_string(arguments, "output")?;
    let variables = arguments
        .get("variables")
        .and_then(Value::as_array)
        .ok_or_else(|| "`variables` is required and must be an array of names".to_string())?;
    let mut names = Vec::new();
    for variable in variables {
        let name = variable
            .as_str()
            .ok_or_else(|| "`variables` must contain strings".to_string())?;
        names.push(name.to_string());
    }
    if names.is_empty() {
        return Err("`variables` must list at least one variable".into());
    }
    crate::results::run(&[
        "export-csv".into(),
        file.into(),
        "--variables".into(),
        names.join(","),
        "--output".into(),
        output.into(),
    ])
    .map_err(|error| error.to_string())
}

fn required_string<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("`{name}` is required and must be a non-empty string"))
}

/// The tool list. Order is fixed so clients and prompt caches see the same thing
/// on every call.
fn tools() -> Value {
    json!([
        {
            "name": "omola_doctor",
            "title": "Check the OpenModelica compiler",
            "description": "Report which OpenModelica compiler Omola uses (OMOLA_OMC or PATH) and its version. Call this first when another tool reports that `omc` is missing.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        },
        {
            "name": "omola_check",
            "title": "Check a Modelica class",
            "description": "Load a `.mo` file and run OpenModelica `checkModel` on one class. Returns the compiler version and diagnostics. Checking is cheaper than simulating and catches syntax and type errors.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model": {"type": "string", "description": "Path to the `.mo` file that defines the class."},
                    "class": {"type": "string", "description": "Fully qualified class name, for example `Modelica.Blocks.Examples.PID_Controller`."}
                },
                "required": ["model", "class"],
                "additionalProperties": false
            }
        },
        {
            "name": "omola_simulate",
            "title": "Simulate a Modelica class",
            "description": "Simulate a class with OpenModelica and return the compiler output, the `.mat` result path, and the path of the run-metadata sidecar. The result can be read with omola_inspect_result or omola_export_csv.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model": {"type": "string", "description": "Path to the `.mo` file that defines the class."},
                    "class": {"type": "string", "description": "Fully qualified class name."},
                    "stop_time": {"type": "number", "description": "Stop time in seconds. Defaults to 1. Must be finite and non-negative."},
                    "parameters": {
                        "type": "object",
                        "description": "Parameter overrides by name, for example {\"gravity\": 9.81}. Values must be numeric.",
                        "additionalProperties": {"type": ["number", "string"]}
                    },
                    "output": {"type": "string", "description": "Path prefix for the result file. OMC appends `_res.mat`."}
                },
                "required": ["model", "class"],
                "additionalProperties": false
            }
        },
        {
            "name": "omola_inspect_result",
            "title": "Inspect a simulation result",
            "description": "Summarise an OpenModelica `.mat` result or an Omola CSV: format, row count, time column, variables, and the run-metadata sidecar when present.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {"type": "string", "description": "Path to a `.mat` or `.csv` result file."}
                },
                "required": ["file"],
                "additionalProperties": false
            }
        },
        {
            "name": "omola_export_csv",
            "title": "Export result variables to CSV",
            "description": "Write selected variables of an OpenModelica `.mat` result to a new CSV file. The time column is added automatically and an existing file is never overwritten.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {"type": "string", "description": "Path to the `.mat` result file."},
                    "variables": {"type": "array", "items": {"type": "string"}, "minItems": 1, "description": "Variable names to export, without the time column."},
                    "output": {"type": "string", "description": "Destination `.csv` path. It must not exist yet."}
                },
                "required": ["file", "variables", "output"],
                "additionalProperties": false
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: u32, method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    fn modern_params() -> Value {
        json!({"_meta": {
            "io.modelcontextprotocol/protocolVersion": MODERN_VERSION,
            "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        }})
    }

    fn send(server: &mut Server, message: &Value) -> Value {
        server
            .handle_line(&message.to_string())
            .expect("requests are answered")
    }

    #[test]
    fn legacy_initialize_declares_tools_and_echoes_a_supported_version() {
        let mut server = Server;
        let reply = send(
            &mut server,
            &request(
                1,
                "initialize",
                json!({"protocolVersion": "2025-06-18", "capabilities": {}}),
            ),
        );
        let result = &reply["result"];
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert_eq!(result["capabilities"]["tools"], json!({}));
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn legacy_initialize_falls_back_to_the_newest_known_revision() {
        let mut server = Server;
        let reply = send(
            &mut server,
            &request(1, "initialize", json!({"protocolVersion": "1900-01-01"})),
        );
        assert_eq!(reply["result"]["protocolVersion"], LEGACY_VERSIONS[0]);
    }

    #[test]
    fn modern_discover_lists_versions_and_identity() {
        let mut server = Server;
        let reply = send(&mut server, &request(1, "server/discover", modern_params()));
        let result = &reply["result"];
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["supportedVersions"][0], MODERN_VERSION);
        assert_eq!(
            result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            SERVER_NAME
        );
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn modern_request_rejects_an_unknown_version_with_the_supported_list() {
        let mut server = Server;
        let params = json!({"_meta": {
            "io.modelcontextprotocol/protocolVersion": "1900-01-01",
            "io.modelcontextprotocol/clientCapabilities": {}
        }});
        let reply = send(&mut server, &request(1, "tools/list", params));
        let error = &reply["error"];
        assert_eq!(error["code"], UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(error["data"]["requested"], "1900-01-01");
        assert_eq!(error["data"]["supported"][0], MODERN_VERSION);
    }

    #[test]
    fn modern_request_requires_client_capabilities() {
        let mut server = Server;
        let params = json!({"_meta": {
            "io.modelcontextprotocol/protocolVersion": MODERN_VERSION
        }});
        let reply = send(&mut server, &request(1, "tools/list", params));
        assert_eq!(reply["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn tools_list_is_stable_and_every_schema_is_an_object() {
        let mut server = Server;
        let reply = send(&mut server, &request(1, "tools/list", json!({})));
        let tools = reply["result"]["tools"].as_array().expect("array");
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("name"))
            .collect();
        let expected: Vec<_> = TOOL_ARGUMENTS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, expected);
        for tool in tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            assert!(tool["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()));
        }
        // No-argument tools must advertise an empty object, not `null`.
        let doctor = &tools[0];
        assert_eq!(doctor["inputSchema"]["additionalProperties"], false);
    }

    #[test]
    fn unknown_method_and_unknown_tool_are_protocol_errors() {
        let mut server = Server;
        let reply = send(&mut server, &request(1, "resources/list", json!({})));
        assert_eq!(reply["error"]["code"], METHOD_NOT_FOUND);

        let reply = send(
            &mut server,
            &request(2, "tools/call", json!({"name": "nope", "arguments": {}})),
        );
        assert_eq!(reply["error"]["code"], INVALID_PARAMS);
        assert!(reply["error"]["message"]
            .as_str()
            .is_some_and(|text| text.contains("Unknown tool")));
    }

    #[test]
    fn unknown_arguments_are_rejected_against_the_published_schema() {
        let mut server = Server;
        let reply = send(
            &mut server,
            &request(
                1,
                "tools/call",
                json!({"name": "omola_check", "arguments": {"model": "a.mo", "class": "A", "verbose": true}}),
            ),
        );
        assert_eq!(reply["error"]["code"], INVALID_PARAMS);
        assert!(reply["error"]["message"]
            .as_str()
            .is_some_and(|text| text.contains("does not take `verbose`")));

        // `omola_doctor` takes no arguments at all.
        let reply = send(
            &mut server,
            &request(
                2,
                "tools/call",
                json!({"name": "omola_doctor", "arguments": {"model": "a.mo"}}),
            ),
        );
        assert_eq!(reply["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn a_parameter_value_cannot_smuggle_in_a_second_override() {
        let dir = std::env::temp_dir().join(format!("omola-mcp-params-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let model = dir.join("Empty.mo");
        std::fs::write(&model, "model Empty\nend Empty;\n").expect("write model");

        let mut server = Server;
        let reply = send(
            &mut server,
            &request(
                1,
                "tools/call",
                json!({
                    "name": "omola_simulate",
                    "arguments": {
                        "model": model.to_string_lossy(),
                        "class": "Empty",
                        "parameters": {"gravity": "9.81,other=1"}
                    }
                }),
            ),
        );
        assert_eq!(reply["result"]["isError"], true);
        assert!(reply["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("finite numeric value")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_arguments_become_tool_errors_not_protocol_errors() {
        let mut server = Server;
        let reply = send(
            &mut server,
            &request(
                1,
                "tools/call",
                json!({"name": "omola_check", "arguments": {}}),
            ),
        );
        let result = &reply["result"];
        assert_eq!(result["isError"], true);
        assert!(result["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("`model` is required")));
    }

    #[test]
    fn notifications_get_no_reply_and_malformed_input_is_reported() {
        let mut server = Server;
        assert!(server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());

        let reply = server.handle_line("{not json").expect("parse error reply");
        assert_eq!(reply["error"]["code"], PARSE_ERROR);
        assert_eq!(reply["id"], Value::Null);
    }

    #[test]
    fn invalid_stop_time_is_rejected_before_omc_is_started() {
        let dir = std::env::temp_dir().join(format!("omola-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let model = dir.join("Empty.mo");
        std::fs::write(&model, "model Empty\nend Empty;\n").expect("write model");

        let mut server = Server;
        let reply = send(
            &mut server,
            &request(
                1,
                "tools/call",
                json!({
                    "name": "omola_simulate",
                    "arguments": {
                        "model": model.to_string_lossy(),
                        "class": "Empty",
                        "stop_time": "soon"
                    }
                }),
            ),
        );
        assert_eq!(reply["result"]["isError"], true);
        assert!(reply["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("stop_time")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_result_reads_a_csv_without_a_compiler() {
        let dir = std::env::temp_dir().join(format!("omola-mcp-csv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let csv = dir.join("run.csv");
        std::fs::write(&csv, "time,height,velocity\n0,1,0\n0.1,0.99,-0.98\n").expect("write csv");

        let mut server = Server;
        let reply = send(
            &mut server,
            &request(
                1,
                "tools/call",
                json!({"name": "omola_inspect_result", "arguments": {"file": csv.to_string_lossy()}}),
            ),
        );
        let result = &reply["result"];
        assert_eq!(result["isError"], false, "{result}");
        let text = result["content"][0]["text"].as_str().expect("text");
        assert!(text.contains("Rows: 2"), "{text}");
        assert!(text.contains("height, velocity"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
