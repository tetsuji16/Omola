# 21 — Serve the Omola workflow over the Model Context Protocol

**Goal:** Let an MCP client (an agent, an IDE assistant) check a class, simulate it, and read the
result without shelling out per command, while keeping OpenModelica as the only compiler and Omola as
orchestration. The normative reference is [`docs/reference/mcp.md`](../reference/mcp.md).

**Depends on:** #1 (process adapter), #2 (smoke model), #6 (results). **Suggested owner:** one
implementation agent.

**Decision (spec):** `omola mcp` is a stdio server, one process per client, no listener and no session
state. It is **dual-era**: it serves the stateless `2026-07-28` revision (`server/discover`, per-request
`_meta`) and the earlier `initialize` handshake down to `2024-11-05`, because clients in the wild are
split across both. Advertised capability is `tools` only — no resources, prompts, subscriptions,
elicitation or logging — so there is nothing to keep in sync per connection.

Five tools, each a thin wrapper over an existing CLI path so the agent sees exactly what the CLI
prints: `omola_doctor`, `omola_check`, `omola_simulate`, `omola_inspect_result`, `omola_export_csv`.
Tool failures are returned as `isError: true` content so the model can correct the request; only
protocol mistakes (unknown method, unknown tool, malformed params) are JSON-RPC errors.

**Work:** Add `crates/omola-cli/src/mcp.rs` with the JSON-RPC loop, era dispatch, tool schemas and
handlers; wire `omola mcp` into `main.rs` and reuse `check` / `simulate` / `results::inspect` instead of
duplicating validation. No new dependencies: the framing is `serde_json` over `std::io`.

**Out of scope:** an HTTP transport, resources/prompts, OAuth, rate limiting, cancellation of an
in-flight OMC run, and any claim of Dymola `simulateModel` equivalence.

**Done when:**
- `omola mcp` serves newline-delimited JSON-RPC on stdin/stdout, writes nothing else to stdout, and
  exits on EOF.
- Modern requests without `clientCapabilities` are rejected with `-32602`; an unsupported version gets
  `-32022` with `error.data.supported`.
- `server/discover` and `initialize` both report the `tools` capability and the server identity.
- Unknown method → `-32601`; unknown tool → `-32602`; a failing tool → `isError: true`.
- Unit tests cover the handshake, version rejection, tool list stability, argument validation and a
  CSV inspection that needs no compiler.
- With OMC installed, check → simulate → inspect → export-csv succeed through the server for
  `examples/BouncingBall.mo`.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo run -p omola -- licenses check`, `git diff --check` pass.

**Validation commands:** `cargo run -p omola -- mcp` with piped JSON-RPC lines; `cargo test -p omola mcp`.
