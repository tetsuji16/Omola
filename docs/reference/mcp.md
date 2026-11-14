# MCP server reference (`omola mcp`)

`omola mcp` exposes the Omola workflow — check a class, simulate it, read the result — to
[Model Context Protocol](https://modelcontextprotocol.io) clients. One process serves one client over
stdio; there is no network listener and no server-side state between requests.

The server is a thin wrapper over the same code paths as the CLI, so an agent sees the same
diagnostics, result paths and run metadata that `omola check` / `omola simulate` / `omola results`
print. Modelica parsing, translation and simulation remain OpenModelica's job; Omola never
reimplements them.

## 1. Running

```sh
omola mcp                 # serve on stdin/stdout
omola mcp --help          # usage on stdout, then exit
```

`omc` must be discoverable exactly as for the CLI: `OMOLA_OMC` first, then `omc` on `PATH`, then the
standard OpenModelica installation directories. A missing compiler is not a startup failure — the
server still answers `tools/list`, and `omola_doctor` reports the problem so the model can react.

A client configuration is a command plus arguments:

```json
{
  "mcpServers": {
    "omola": {"command": "omola", "args": ["mcp"]}
  }
}
```

## 2. Transport

The stdio binding is newline-delimited JSON-RPC 2.0:

- One JSON message per line, UTF-8, **no embedded newlines**.
- `stdout` carries nothing but MCP messages. Diagnostics, if any, go to `stderr`.
- The server exits when `stdin` reaches end of file. It does not need a shutdown request.
- Requests are handled synchronously and in order; a tool call finishes before the next line is read.

Set `OMOLA_MCP_LOG=1` to echo received notifications to `stderr` while debugging a client. Nothing is
logged otherwise.

## 3. Protocol versions

The server is **dual-era**. It selects the era per request, as `2026-07-28` §Versioning describes.

| Era | Versions | How it is detected | Handshake |
| --- | --- | --- | --- |
| Modern | `2026-07-28` | the request carries `_meta["io.modelcontextprotocol/protocolVersion"]` | none; `server/discover` reports capabilities |
| Legacy | `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05` | no such `_meta`, or an `initialize` request | `initialize` → `notifications/initialized` |

Rules the server enforces:

- A modern request whose version is not in the list above is rejected with
  `-32022 UnsupportedProtocolVersion`, and `error.data.supported` lists what the server does support.
- A modern request without `_meta["io.modelcontextprotocol/clientCapabilities"]` is rejected with
  `-32602 Invalid params`; `2026-07-28` makes that field mandatory.
- A legacy `initialize` echoes the requested version when it is supported and otherwise answers with
  `2025-11-25`, the newest legacy revision the server knows.
- `server/discover` returns `supportedVersions`, `capabilities`, `_meta["…/serverInfo"]` and
  `instructions`. Modern results carry `"resultType": "complete"`; legacy results do not.

Advertised capabilities are `{"tools": {}}` only. There are no resources, prompts, subscriptions,
elicitation, sampling or logging features, so `listChanged` is absent: the tool set is fixed and never
changes during a session.

## 4. Tools

The list is returned in a fixed order so clients and prompt caches see the same thing on every call.

| Tool | Arguments | Returns |
| --- | --- | --- |
| `omola_doctor` | — | The compiler Omola resolved and its version |
| `omola_check` | `model`, `class` | `checkModel` diagnostics |
| `omola_simulate` | `model`, `class`, `stop_time`?, `parameters`?, `output`? | Compiler output, `.mat` result path, run-metadata path |
| `omola_inspect_result` | `file` | Format, row count, variables, metadata sidecar |
| `omola_export_csv` | `file`, `variables`, `output` | Destination path and row count |

A missing or `null` `arguments` is treated as an empty object, so a tool that needs none can be called
without one. Otherwise each tool declares `additionalProperties: false`, so an argument it does not
list in the bullets below is rejected with `-32602` rather than ignored:

- `model` is a path to a `.mo` file on the machine running the server. It must exist.
- `class` is a fully qualified Modelica name, for example `Modelica.Blocks.Examples.PID_Controller`.
- `stop_time` is a finite, non-negative number of seconds and defaults to `1`.
- `parameters` maps parameter names to numeric values (`9.81` and `"9.81"` are both accepted). Names and
  values are validated as the CLI validates `--parameters`; the same request always produces the same
  OMC script because the keys are sorted.
- `output` is a path prefix. OMC appends `_res.mat`, and the run-metadata sidecar is written next to it.
- `variables` must be unique, non-empty, and must not include `time`, which is added automatically.
- `output` for `omola_export_csv` must be a `.csv` path that does not exist yet. The server never
  overwrites a file.

### Error model

Two mechanisms, as the specification requires:

| Situation | Reply |
| --- | --- |
| Unknown method | JSON-RPC error `-32601` |
| Unknown tool, missing `name`, malformed `arguments`, or an argument name the tool does not declare | JSON-RPC error `-32602` |
| Unparseable line | JSON-RPC error `-32700`, `id: null` |
| Tool ran but failed (missing compiler, Modelica error, bad path, refused overwrite) | result with `"isError": true` and the diagnostic as text |

Tool failures are returned inside a normal result so the model can read the message and correct the
request — for example by fixing a path after `result file does not exist`.

## 5. Example session

Modern client (`2026-07-28`). Every request repeats the same `_meta` block, which is what makes the
era detection work — a `tools/call` without it would be served as a legacy request. Abbreviated:

```json
{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}},"name":"omola_simulate","arguments":{"model":"examples/BouncingBall.mo","class":"BouncingBall","stop_time":1,"parameters":{"gravity":9.81}}}}
```

```json
{"id":1,"jsonrpc":"2.0","result":{"resultType":"complete","supportedVersions":["2026-07-28","2025-11-25","2025-06-18","2025-03-26","2024-11-05"],"capabilities":{"tools":{}},"_meta":{"io.modelcontextprotocol/serverInfo":{"name":"omola","version":"0.1.1"}},"instructions":"…"}}
{"id":2,"jsonrpc":"2.0","result":{"resultType":"complete","content":[{"type":"text","text":"OpenModelica v1.27.1 (64-bit)\nResult: …/bb_res.mat\nRun metadata: …/bb_res.json\n…"}],"isError":false}}
```

The text of a simulation result is the CLI output, so it contains the compiler version, the result
path, the metadata path and OMC's own log. Passing that result to `omola_inspect_result` or
`omola_export_csv` is the intended follow-up.

## 6. Limits and security

- **The client is trusted.** A tool call runs `omc` and reads or writes any path the caller names,
  with the privileges of the user who started the server. Do not connect clients you would not let run
  `omola simulate` in a shell.
- **stdio only.** There is no HTTP transport, so the server is reachable only by the process that
  launched it. Nothing listens on a port.
- **No cancellation.** `notifications/cancelled` is accepted and ignored: every call is synchronous and
  bounded by OMC's own runtime, and a half-written result file would be worse than waiting.
- **No sandbox or rate limit.** Long simulations block the session until OMC returns.
- **No Dymola parity claim.** The tools expose measured Omola behaviour; `omola_simulate` is not a
  drop-in replacement for a Dymola `simulateModel` call.
