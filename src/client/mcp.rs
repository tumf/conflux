//! `cflx client mcp` — the client boundary, spoken as Model Context Protocol.
//!
//! # What this is
//!
//! A stdio JSON-RPC frontend over exactly the control and subscription boundary
//! `cflx client` already offers. It is a *client*: it takes no repository lock,
//! binds no listener, starts no run, and holds no orchestration state. Every
//! tool call resolves a connection, asks the owner, and returns the same
//! versioned envelope the CLI prints.
//!
//! # Why three tools rather than six
//!
//! The previous surface exposed its own implementation history — enqueue, wait,
//! and three notify verbs — and one of those tools chose admission policy on the
//! model's behalf. An agent needs three things: read the owner, control it the
//! way an operator does, and ask to be told when a proposal finishes. So the
//! tools are `cflx_status`, `cflx_control`, and `cflx_subscribe`, and the
//! actions inside `cflx_control` are exactly the operator's: mark, unmark,
//! start, stop, force_stop, force_stop_change.
//!
//! # Why the tool set is closed
//!
//! The obvious design — expose `/api/v2` and let the model compose commands — is
//! the one thing this must not do. `POST /api/v2/commands` needs an expected
//! revision, an idempotency key, and mode-aware routing; a model that got any of
//! those wrong would consume another operator's execution marks or double-submit
//! under a stale revision. So the tools are intents, the routing stays in
//! `control`, and there is no way to name a raw command type, revision,
//! idempotency key, queue intent, or shell string at all.
//!
//! # Why stdout is protocol-only
//!
//! An MCP host parses stdout as a JSON-RPC stream. A single stray log line
//! desynchronizes the session, so nothing here writes to stdout except complete
//! frames, and every diagnostic goes to stderr.
//!
//! # Bounded calls, and why `cflx_wait` is not here
//!
//! `cflx_control` returns as soon as its commands settle and `cflx_subscribe` as
//! soon as the registry does; neither holds a call open for the life of a
//! proposal. A completion wait is the opposite shape — it is open for exactly as
//! long as the work takes — so it stays a CLI command. An MCP host that wants
//! asynchronous completion registers an explicit callback with `cflx_subscribe`;
//! a host that cannot execute one has no MCP completion oracle, by design.

use std::path::PathBuf;

use serde::Deserialize;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::RouteSelector;

/// The MCP revision this adapter implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Revisions this adapter will echo back when a client asks for one of them.
///
/// A client that asks for something else is answered with [`PROTOCOL_VERSION`],
/// which the MCP handshake defines as the server proposing its own revision
/// rather than failing the session.
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 2] = ["2025-06-18", "2025-03-26"];

/// Longest accepted single JSON-RPC frame.
///
/// A stdio peer that never emits a newline would otherwise grow the read buffer
/// without bound, so the bound is enforced *while bytes are read* rather than
/// after a newline finally arrives. The ceiling is far above any legitimate tool
/// call.
const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Every tool this adapter exposes, in listing order.
pub const TOOL_NAMES: [&str; 3] = ["cflx_status", "cflx_control", "cflx_subscribe"];

/// Connection settings every tool accepts.
///
/// A token *value* is deliberately absent: the field names an environment
/// variable, so nothing that can read this process's arguments — or the model's
/// own transcript — ever sees a credential.
///
/// `project_dir` is the normal public selector and `unix_socket` the low-level
/// override. Both are optional, and both are call-scoped: nothing a call
/// supplies is remembered, so one server process can serve several projects
/// without holding a project-to-socket map that could go stale between turns.
#[derive(Debug, Clone, Default, Deserialize)]
struct ConnectionArgs {
    #[serde(default)]
    project_dir: Option<PathBuf>,
    #[serde(default)]
    unix_socket: Option<PathBuf>,
    #[serde(default)]
    auth_token_env: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusArgs {
    #[serde(flatten)]
    connection: ConnectionArgs,
}

/// `cflx_control` arguments.
///
/// One tool with an action rather than six tools, because the six are one
/// decision an operator makes: which of the controls the TUI offers to use. The
/// target list is optional at the parsing layer and required in a different
/// shape by each family — 1..64 for the two mark actions, exactly one for
/// `force_stop_change`, none for the three process-wide lifecycle actions — so
/// an action carrying the wrong one is refused as a usage error rather than
/// silently ignoring it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlArgs {
    action: String,
    #[serde(default)]
    change_ids: Vec<String>,
    #[serde(flatten)]
    connection: ConnectionArgs,
}

/// `cflx_subscribe` arguments.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubscribeArgs {
    action: String,
    change_ids: Vec<String>,
    /// Owner incarnation the caller believes it is addressing, when it kept one.
    #[serde(default)]
    instance_id: Option<String>,
    /// Callback argv. Required by `set`, refused by `get` and `clear`.
    #[serde(default)]
    command: Option<Vec<String>>,
    #[serde(default)]
    notify_blocked: bool,
    #[serde(flatten)]
    connection: ConnectionArgs,
}

/// Why a tool call could not be attempted at all.
///
/// Distinct from an unsuccessful envelope: a rejected argument list never
/// reached the owner, so reporting it as a client outcome would claim an
/// observation that never happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    /// Sanitized explanation. Never carries a credential.
    pub message: String,
}

impl ToolError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// What actually talks to an owner.
///
/// Behind a trait so the protocol layer is testable without a socket: framing,
/// negotiation, schema exposure, and error mapping are contract, and none of
/// them should need a running owner to assert.
#[async_trait::async_trait]
pub trait ToolHost: Send + Sync {
    /// Run one named tool and return the envelope it produced.
    async fn call(
        &self,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ResultEnvelope, ToolError>;
}

/// The production host: every call goes through the same client modules the CLI
/// uses, so a tool and a command cannot disagree about routing or truthfulness.
pub struct ClientToolHost {
    default_route: RouteSelector,
    default_token_env: Option<String>,
}

impl ClientToolHost {
    /// Build a host whose per-call connection settings default to the ones the
    /// `cflx client` namespace was invoked with.
    pub fn new(default_route: RouteSelector, default_token_env: Option<String>) -> Self {
        Self {
            default_route,
            default_token_env,
        }
    }

    /// Decide this call's route, or refuse it before any owner is contacted.
    ///
    /// A [`ToolError`] rather than an envelope on purpose. The two refusals
    /// this can produce — two selectors in one call, and a path that is not a
    /// usable Git working tree — are complaints about the *arguments*, and no
    /// socket was opened for either. Reporting them as a client outcome would
    /// claim an owner conversation that never happened, and adding a new
    /// envelope outcome for them would put a validation failure into the stable
    /// result contract.
    ///
    /// The call-scoped selector shadows [`Self::default_route`] rather than
    /// replacing it, so a namespace default survives a call that overrode it.
    fn route(&self, args: &ConnectionArgs) -> Result<RouteSelector, ToolError> {
        let selector =
            RouteSelector::from_inputs(args.project_dir.as_deref(), args.unix_socket.as_deref())
                .map_err(|error| ToolError::new(error.message))?;
        let selector = selector.or_default(&self.default_route);
        // Resolved eagerly so an unusable project is a bounded validation
        // failure here, before contact, rather than a connection refusal that
        // reads as though an owner answered.
        if let RouteSelector::Project(project_dir) = &selector {
            crate::client::resolve_project(project_dir)
                .map_err(|error| ToolError::new(error.message))?;
        }
        Ok(selector)
    }

    /// Resolve a connection, or return the refusal envelope to answer with.
    ///
    /// `Box`ed because the envelope is much larger than the connection handle,
    /// and an unboxed error variant would make every successful call carry the
    /// failure's footprint.
    fn connect(
        &self,
        route: &RouteSelector,
        args: &ConnectionArgs,
        operation: Operation,
    ) -> Result<crate::client::session::Connection, Box<ResultEnvelope>> {
        let token_env = args
            .auth_token_env
            .clone()
            .or_else(|| self.default_token_env.clone());
        crate::client::session::Connection::resolve_route(route, token_env.as_deref())
            .map_err(|refusal| Box::new(refusal.into_envelope(operation)))
    }
}

#[async_trait::async_trait]
impl ToolHost for ClientToolHost {
    async fn call(
        &self,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ResultEnvelope, ToolError> {
        match name {
            "cflx_status" => {
                let args: StatusArgs = parse_args(arguments)?;
                let route = self.route(&args.connection)?;
                let connection = match self.connect(&route, &args.connection, Operation::Status) {
                    Ok(connection) => connection,
                    Err(envelope) => return Ok(*envelope),
                };
                Ok(crate::client::session::status(&connection).await)
            }
            "cflx_control" => {
                let args: ControlArgs = parse_args(arguments)?;
                let action =
                    crate::client::control::Action::parse(&args.action).ok_or_else(|| {
                        ToolError::new(format!(
                            "'{}' is not a control action; use one of mark, unmark, start, stop, \
                             force_stop, or force_stop_change",
                            args.action
                        ))
                    })?;
                let change_ids = validated_change_ids(&args.change_ids)?;
                let operation = action.operation();
                // Shape refusals are answered as the stable `usage_error`
                // envelope rather than as a protocol error, because they are
                // facts about the request a model has to read and correct — and
                // they happen here, before any route is even resolved.
                if let Err(message) = crate::client::control::validate_request(action, &change_ids)
                {
                    return Ok(usage_error(operation, message));
                }
                let route = self.route(&args.connection)?;
                let connection = match self.connect(&route, &args.connection, operation) {
                    Ok(connection) => connection,
                    Err(envelope) => return Ok(*envelope),
                };
                Ok(crate::client::control::run(&connection, action, &change_ids).await)
            }
            "cflx_subscribe" => {
                let args: SubscribeArgs = parse_args(arguments)?;
                let change_ids = validated_change_ids(&args.change_ids)?;
                let intent = match args.action.as_str() {
                    "set" => crate::client::subscribe::Intent::Set {
                        // An absent argv is an empty one on purpose: the shared
                        // validator already refuses that with the reason a
                        // caller needs, and a second spelling of the same
                        // refusal would be a second thing to keep in step.
                        command: args.command.clone().unwrap_or_default(),
                        notify_blocked: args.notify_blocked,
                    },
                    "get" => crate::client::subscribe::Intent::Get,
                    "clear" => crate::client::subscribe::Intent::Clear,
                    other => {
                        return Err(ToolError::new(format!(
                            "'{other}' is not a subscription action; use one of set, get, or clear"
                        )))
                    }
                };
                let operation = intent.operation();
                // A callback on a read or a removal is a caller that believes it
                // is registering something. Ignoring it would let that belief
                // survive the call.
                if args.command.is_some()
                    && !matches!(intent, crate::client::subscribe::Intent::Set { .. })
                {
                    return Ok(usage_error(
                        operation,
                        format!(
                            "the '{}' action runs no callback, so it accepts no command argv",
                            args.action
                        ),
                    ));
                }
                if let Err(message) =
                    crate::client::subscribe::validate_request(&change_ids, &intent)
                {
                    return Ok(usage_error(operation, message));
                }
                let route = self.route(&args.connection)?;
                let connection = match self.connect(&route, &args.connection, operation) {
                    Ok(connection) => connection,
                    Err(envelope) => return Ok(*envelope),
                };
                Ok(crate::client::subscribe::run(
                    &connection,
                    &change_ids,
                    args.instance_id.as_deref(),
                    intent,
                )
                .await)
            }
            other => Err(ToolError::new(format!(
                "'{other}' is not a tool this server exposes"
            ))),
        }
    }
}

/// The stable usage-failure envelope for a request this server refused itself.
fn usage_error(operation: Operation, message: impl Into<String>) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::UsageError).with_message(message)
}

/// Parse one tool's arguments, failing closed on anything unexpected.
fn parse_args<T: serde::de::DeserializeOwned>(
    arguments: &serde_json::Value,
) -> Result<T, ToolError> {
    let arguments = if arguments.is_null() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        arguments.clone()
    };
    serde_json::from_value(arguments).map_err(|error| {
        // The model's own text is never echoed back: only serde's structural
        // complaint, which names fields rather than values.
        ToolError::new(format!("the tool arguments were not accepted: {error}"))
    })
}

/// Apply the CLI's own change-ID rule to a tool argument.
///
/// The same narrow shape, for the same reason: a change ID reaches a URL query
/// and a Git ref derivation, and a model is exactly the kind of caller that
/// would send `../` if nothing stopped it.
fn validated_change_id(value: &str) -> Result<String, ToolError> {
    crate::cli::parse_change_id(value).map_err(ToolError::new)
}

/// Apply that rule to every element of a target list.
///
/// Shape only. Count and distinctness belong to the shared request validators,
/// so the two surfaces cannot drift: a list the CLI would refuse must not be one
/// a tool accepts.
fn validated_change_ids(values: &[String]) -> Result<Vec<String>, ToolError> {
    values
        .iter()
        .map(|value| validated_change_id(value))
        .collect()
}

/// The published `tools/list` payload.
pub fn tool_descriptors() -> serde_json::Value {
    let connection_properties = serde_json::json!({
        "project_dir": {
            "type": "string",
            "description": "Absolute directory inside the project whose owner to talk to. The normal selector: it names the project rather than one owner incarnation's transport, so a single server process can serve several projects by passing a different directory per call. A linked worktree, a submodule, or any directory below the working-tree root resolves. The owner socket and the repository that certifies completion both come from this project. Mutually exclusive with unix_socket in one call."
        },
        "unix_socket": {
            "type": "string",
            "description": "Low-level override: path to the owner's /api/v2 Unix socket, for diagnostics, tests, and non-repository transports. Prefer project_dir. Defaults to ${GIT_COMMON_DIR}/cflx-api.sock. Mutually exclusive with project_dir in one call."
        },
        "auth_token_env": {
            "type": "string",
            "description": "Name of an environment variable holding the bearer token. A token value is never accepted."
        }
    });
    let with_connection = |mut properties: serde_json::Value| {
        if let (Some(target), Some(source)) = (
            properties.as_object_mut(),
            connection_properties.as_object(),
        ) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        properties
    };

    serde_json::json!({
        "tools": [
            {
                "name": "cflx_status",
                "title": "Read the existing Conflux owner",
                "description": "Read one coherent snapshot of the Conflux process that owns this repository: owner incarnation, application mode, scheduler and activity state, per-proposal status, execution marks, queue intent, and execution episodes. Mutates nothing and submits no command.",
                "inputSchema": {
                    "type": "object",
                    "properties": with_connection(serde_json::json!({})),
                    "additionalProperties": false
                }
            },
            {
                "name": "cflx_control",
                "title": "Control the owner exactly as an operator does",
                "description": "Do one of the six things a Conflux operator does at the TUI. 'mark' and 'unmark' set or clear the named proposals' execution marks: target-scoped desired-state writes that preserve every unrelated mark, submit no queue intent, start nothing, and return without waiting for admission — the owner's own settlement decides whether stable marked work later runs. 'start', 'stop', and 'force_stop' submit the shared lifecycle intents F5/'!' and the stop controls submit; 'start' consumes the marks the owner already holds and takes no target list. 'force_stop_change' names exactly one proposal in change_ids and kills that one: the owner SIGKILLs the managed process group it owns without the graceful SIGTERM window 'stop' gives it, waits for confirmed reaping, then dequeues it and clears its execution mark, leaving every unrelated change and the process-wide run mode untouched. It is never a way to stop everything — that is 'force_stop'. Branch on the envelope's `outcome`: marked, unmarked, unchanged, stopped, and accepted are the successes.",
                "inputSchema": {
                    "type": "object",
                    "properties": with_connection(serde_json::json!({
                        "action": {
                            "type": "string",
                            "enum": ["mark", "unmark", "start", "stop", "force_stop", "force_stop_change"],
                            "description": "Which operator control to use."
                        },
                        "change_ids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "minItems": 1,
                            "maxItems": 64,
                            "description": "Proposals this call addresses. mark and unmark take 1 through 64 distinct proposals. force_stop_change takes exactly one — zero, several, duplicate, or blank targets are refused before anything is contacted. start, stop, and force_stop take none: they consume the owner's authoritative mark set."
                        }
                    })),
                    "required": ["action"],
                    "additionalProperties": false
                }
            },
            {
                "name": "cflx_subscribe",
                "title": "Ask to be told when named proposals finish",
                "description": "Register, read, or remove completion callbacks for named proposals. A subscription is keyed by the proposal, so it can be registered before the owner admits anything; whenever a subscribed proposal enters a new execution episode the owner binds it and delivers that episode's first terminal classification — completed, failed, or stopped — once. Re-admission after a retry is a distinct episode and a distinct notification. Registering after the latest episode already settled delivers that event immediately, and never again. Delivery is notification only: Conflux runs the registered argv and does not start, resume, or message an agent, and the callback's exit status changes no workflow outcome. Registering mutates no workflow state, creates no command record, and advances no revision. Subscriptions are process-local: an owner restart invalidates all of them.",
                "inputSchema": {
                    "type": "object",
                    "properties": with_connection(serde_json::json!({
                        "action": {
                            "type": "string",
                            "enum": ["set", "get", "clear"],
                            "description": "Register or replace, inspect, or remove."
                        },
                        "change_ids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "minItems": 1,
                            "maxItems": 64,
                            "description": "Proposals to address: 1 through 64, all distinct. There is no list-all."
                        },
                        "instance_id": {
                            "type": "string",
                            "description": "Owner incarnation cflx_status reported. Supplying it turns an owner restart into a typed owner_restarted refusal instead of a silent registration against a process that never saw your work."
                        },
                        "command": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Callback argv, required by 'set' and refused by 'get' and 'clear'. Element 0 is the program; no shell interpretation is applied. Accepted only over the owner's Unix socket."
                        },
                        "notify_blocked": {
                            "type": "boolean",
                            "description": "Also deliver the non-terminal blocked attention edge. Terminal events are always delivered and cannot be disabled."
                        }
                    })),
                    "required": ["action", "change_ids"],
                    "additionalProperties": false
                }
            }
        ]
    })
}

/// JSON-RPC error codes this adapter emits.
mod rpc {
    /// The frame was not valid JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// The frame was JSON but not a JSON-RPC request.
    pub const INVALID_REQUEST: i64 = -32600;
    /// No such method.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The method exists but the handshake has not completed.
    ///
    /// In the implementation-defined range on purpose: a host that skipped
    /// `initialize` needs to tell "you have not initialized" apart from "no such
    /// method", and neither of the standard codes says that.
    pub const SERVER_NOT_INITIALIZED: i64 = -32002;
}

/// Whether a JSON-RPC `id` member is one this server can echo back.
///
/// JSON-RPC 2.0 allows a string, a number, or `null`. An object or an array is
/// an invalid request object, and the response to one has to carry `null`
/// rather than the malformed value.
fn is_usable_id(value: &serde_json::Value) -> bool {
    matches!(
        value,
        serde_json::Value::Null | serde_json::Value::String(_) | serde_json::Value::Number(_)
    )
}

/// One MCP session over a pair of byte streams.
pub struct Session<H: ToolHost> {
    host: H,
    /// True once this server has *answered* `initialize`. Tool listing and tool
    /// calls are gated on it: a host that skipped the handshake is a protocol
    /// violation worth naming rather than serving, and an unanswered handshake
    /// enables nothing.
    initialized: bool,
}

impl<H: ToolHost> Session<H> {
    /// Build a session over one tool host.
    pub fn new(host: H) -> Self {
        Self {
            host,
            initialized: false,
        }
    }

    /// Whether `initialize` has been answered.
    // Observed by the handshake assertions, and by the tool gate below.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Handle one JSON-RPC frame, returning the frame to write back.
    ///
    /// `None` means the message was a notification, which by JSON-RPC has no
    /// response at all — writing one would desynchronize a strict host. That
    /// holds for an *invalid* notification too: a malformed message with no
    /// `id` is still a message this server may not answer.
    pub async fn handle(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let message: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(message) => message,
            Err(error) => {
                return Some(error_frame(
                    serde_json::Value::Null,
                    rpc::PARSE_ERROR,
                    format!("the frame is not valid JSON: {error}"),
                ))
            }
        };

        // Batch support is not advertised, so an array is an invalid request
        // rather than a set of calls this server would half-answer.
        if message.is_array() {
            return Some(error_frame(
                serde_json::Value::Null,
                rpc::INVALID_REQUEST,
                "this server does not accept JSON-RPC batches; send one request object per line",
            ));
        }
        let Some(object) = message.as_object() else {
            return Some(error_frame(
                serde_json::Value::Null,
                rpc::INVALID_REQUEST,
                "a JSON-RPC message must be an object",
            ));
        };

        // An `id` member that this server cannot echo makes the whole request
        // object invalid, and JSON-RPC 2.0 reserves `null` for answering it.
        let raw_id = object.get("id");
        if raw_id.is_some_and(|value| !is_usable_id(value)) {
            return Some(error_frame(
                serde_json::Value::Null,
                rpc::INVALID_REQUEST,
                "a JSON-RPC id must be a string, a number, or null",
            ));
        }
        // Absent `id` means notification: no response, whatever else is wrong.
        let respond = raw_id.cloned();

        if object.get("jsonrpc").and_then(|value| value.as_str()) != Some("2.0") {
            return respond.map(|id| {
                error_frame(
                    id,
                    rpc::INVALID_REQUEST,
                    "every frame must identify JSON-RPC 2.0 with \"jsonrpc\": \"2.0\"",
                )
            });
        }

        let Some(method) = object.get("method").and_then(|value| value.as_str()) else {
            return respond
                .map(|id| error_frame(id, rpc::INVALID_REQUEST, "the frame carries no method"));
        };
        let params = object
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let Some(id) = respond else {
            // `notifications/initialized` is accepted idempotently and is never
            // a substitute for the handshake: only this server's own successful
            // `initialize` response enables the tools.
            return None;
        };

        match method {
            "initialize" => {
                let result = self.initialize_result(&params);
                self.initialized = true;
                Some(result_frame(id, result))
            }
            "ping" => Some(result_frame(id, serde_json::json!({}))),
            "tools/list" | "tools/call" if !self.initialized => Some(error_frame(
                id,
                rpc::SERVER_NOT_INITIALIZED,
                format!("'{method}' is served only after a successful initialize handshake"),
            )),
            "tools/list" => Some(result_frame(id, tool_descriptors())),
            "tools/call" => Some(result_frame(id, self.call_tool(&params).await)),
            other => Some(error_frame(
                id,
                rpc::METHOD_NOT_FOUND,
                format!("'{other}' is not a method this server implements"),
            )),
        }
    }

    fn initialize_result(&self, params: &serde_json::Value) -> serde_json::Value {
        // Echo the client's revision when it is one this adapter speaks;
        // otherwise propose our own, which is what the handshake is for.
        let requested = params
            .get("protocolVersion")
            .and_then(|value| value.as_str())
            .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
            .unwrap_or(PROTOCOL_VERSION);
        serde_json::json!({
            "protocolVersion": requested,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {
                "name": "cflx-client",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": "Control client of the Conflux process that already owns this \
                             repository, with the operator's own verbs. cflx_control mark selects \
                             proposals — it preserves unrelated marks and claims no admission — \
                             and cflx_control start is the F5 equivalent that consumes the \
                             owner's authoritative mark set. Neither is completion. For \
                             asynchronous completion, register a callback explicitly with \
                             cflx_subscribe set; nothing is registered for you, and delivery \
                             notifies rather than resumes. There is no wait tool: use \
                             `cflx client wait` when a bounded synchronous observation is what \
                             you need. Branch on the envelope's `outcome` field, never on prose.",
        })
    }

    /// Run one `tools/call`.
    ///
    /// A refused argument list and an unsuccessful owner outcome are both
    /// `isError: true` tool results rather than JSON-RPC errors, because the
    /// *call* succeeded — the model needs to read the reason and decide, which a
    /// protocol-level error would hide from it.
    async fn call_tool(&self, params: &serde_json::Value) -> serde_json::Value {
        let Some(name) = params.get("name").and_then(|value| value.as_str()) else {
            return tool_failure("the call names no tool");
        };
        if !TOOL_NAMES.contains(&name) {
            return tool_failure(format!("'{name}' is not a tool this server exposes"));
        }
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match self.host.call(name, &arguments).await {
            Ok(envelope) => {
                let value =
                    serde_json::to_value(&envelope).unwrap_or_else(|_| serde_json::json!({}));
                serde_json::json!({
                    "content": [{"type": "text", "text": envelope.to_json_line()}],
                    "structuredContent": value,
                    "isError": !envelope.ok,
                })
            }
            Err(error) => tool_failure(error.message),
        }
    }
}

/// Build the `isError` tool result for a call that never reached an owner.
fn tool_failure(message: impl Into<String>) -> serde_json::Value {
    let message = message.into();
    serde_json::json!({
        "content": [{"type": "text", "text": message}],
        "isError": true,
    })
}

fn result_frame(id: serde_json::Value, result: serde_json::Value) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"result is not encodable"}}"#
            .to_string()
    })
}

fn error_frame(id: serde_json::Value, code: i64, message: impl Into<String>) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message.into()},
    }))
    .unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"error is not encodable"}}"#
            .to_string()
    })
}

/// What one bounded framing attempt produced.
enum Frame {
    /// A complete line, without its newline terminator.
    Line(String),
    /// The peer closed the stream with nothing buffered.
    Eof,
    /// The frame can never be interpreted, so the session ends.
    Unusable(String),
    /// The stream itself failed.
    Failed(std::io::Error),
}

/// Read one newline-delimited frame, never retaining more than the bound.
///
/// `read_line` cannot do this: it grows its `String` until a newline arrives, so
/// a peer that sends a gigabyte without one has already been buffered by the
/// time any ceiling could be checked. Here the limit is enforced against every
/// chunk as it is taken from the reader, so retained input stays inside
/// [`MAX_FRAME_BYTES`] plus the reader's own fixed buffer.
///
/// An oversized or non-UTF-8 frame is terminal for the session. There is no
/// resynchronization: the remaining bytes belong to a frame this server already
/// refused to hold, and guessing where the next one starts is how a desynchronized
/// stream turns into a dispatched tool call nobody sent.
async fn read_frame<R>(reader: &mut R, buffer: &mut Vec<u8>) -> Frame
where
    R: AsyncBufRead + Unpin,
{
    /// What one pass over the reader's buffer accomplished.
    enum Step {
        /// A newline was found; the frame is complete.
        Complete,
        /// Bytes were retained; keep reading.
        Partial,
        /// The peer closed the stream.
        Eof,
        /// Retaining this chunk would exceed the bound.
        Overflow,
    }

    buffer.clear();
    loop {
        let (step, used) = {
            let available = match reader.fill_buf().await {
                Ok(available) => available,
                Err(error) => return Frame::Failed(error),
            };
            if available.is_empty() {
                (Step::Eof, 0)
            } else if let Some(index) = available.iter().position(|byte| *byte == b'\n') {
                if buffer.len() + index > MAX_FRAME_BYTES {
                    (Step::Overflow, 0)
                } else {
                    buffer.extend_from_slice(&available[..index]);
                    (Step::Complete, index + 1)
                }
            } else if buffer.len() + available.len() > MAX_FRAME_BYTES {
                (Step::Overflow, 0)
            } else {
                let len = available.len();
                buffer.extend_from_slice(available);
                (Step::Partial, len)
            }
        };
        reader.consume(used);
        match step {
            Step::Partial => continue,
            Step::Overflow => {
                buffer.clear();
                return Frame::Unusable(format!(
                    "a frame exceeded {MAX_FRAME_BYTES} bytes without a newline"
                ));
            }
            Step::Complete => return decode(buffer),
            Step::Eof => {
                return if buffer.is_empty() {
                    Frame::Eof
                } else {
                    // A final frame the peer never terminated is still one frame.
                    decode(buffer)
                };
            }
        }
    }
}

/// Turn retained bytes into a frame, keeping the buffer's capacity.
fn decode(buffer: &[u8]) -> Frame {
    match std::str::from_utf8(buffer) {
        Ok(line) => Frame::Line(line.to_string()),
        Err(_) => Frame::Unusable("a frame was not valid UTF-8".to_string()),
    }
}

/// Serve one stdio MCP session and return the process exit status.
///
/// Reads newline-delimited JSON-RPC from `input` and writes newline-delimited
/// JSON-RPC to `output`, and nothing else to `output` ever.
pub async fn serve<H, R, W>(host: H, input: R, mut output: W) -> i32
where
    H: ToolHost,
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut session = Session::new(host);
    let mut reader = BufReader::new(input);
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let line = match read_frame(&mut reader, &mut buffer).await {
            Frame::Line(line) => line,
            Frame::Eof => break,
            Frame::Unusable(reason) => {
                eprintln!("cflx client mcp: {reason}; the session ends unread");
                return Outcome::TransportError.exit_code();
            }
            Frame::Failed(error) => {
                eprintln!("cflx client mcp: stdin failed: {error}");
                return Outcome::TransportError.exit_code();
            }
        };
        let Some(response) = session.handle(&line).await else {
            continue;
        };
        if output
            .write_all(response.as_bytes())
            .await
            .and(output.write_all(b"\n").await)
            .is_err()
            || output.flush().await.is_err()
        {
            eprintln!("cflx client mcp: the host closed the protocol stream");
            return Outcome::TransportError.exit_code();
        }
    }
    0
}

/// Run `cflx client mcp` over the process's own stdio.
pub async fn run(default_route: RouteSelector, default_token_env: Option<String>) -> i32 {
    serve(
        ClientToolHost::new(default_route, default_token_env),
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A host that answers from a script, so protocol behavior is assertable
    /// without a socket, a repository, or an owner.
    #[derive(Default)]
    struct FakeHost {
        calls: Mutex<Vec<(String, serde_json::Value)>>,
        answer: Mutex<Option<Result<ResultEnvelope, ToolError>>>,
    }

    impl FakeHost {
        fn answering(envelope: ResultEnvelope) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                answer: Mutex::new(Some(Ok(envelope))),
            }
        }

        fn refusing(message: &str) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                answer: Mutex::new(Some(Err(ToolError::new(message)))),
            }
        }
    }

    #[async_trait::async_trait]
    impl ToolHost for FakeHost {
        async fn call(
            &self,
            name: &str,
            arguments: &serde_json::Value,
        ) -> Result<ResultEnvelope, ToolError> {
            self.calls
                .lock()
                .unwrap()
                .push((name.to_string(), arguments.clone()));
            self.answer
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(ToolError::new("no scripted answer")))
        }
    }

    fn parse(frame: &str) -> serde_json::Value {
        serde_json::from_str(frame).expect("every response frame must be one JSON object")
    }

    async fn respond(
        session: &mut Session<FakeHost>,
        request: serde_json::Value,
    ) -> serde_json::Value {
        let frame = session
            .handle(&request.to_string())
            .await
            .expect("a request with an id owes a response");
        assert!(!frame.contains('\n'), "a frame must be one line: {frame}");
        parse(&frame)
    }

    fn session() -> Session<FakeHost> {
        Session::new(FakeHost::default())
    }

    /// A session that has completed the handshake, which is what every tool
    /// assertion needs now that the gate is real.
    async fn initialized(host: FakeHost) -> Session<FakeHost> {
        let mut session = Session::new(host);
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": {"protocolVersion": PROTOCOL_VERSION, "capabilities": {}}
            }),
        )
        .await;
        assert!(response["result"].is_object());
        assert!(session.is_initialized());
        session
    }

    #[tokio::test]
    async fn initialize_echoes_a_revision_it_speaks_and_proposes_its_own_otherwise() {
        let mut session = session();
        assert!(!session.is_initialized());
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-03-26", "capabilities": {}}
            }),
        )
        .await;
        assert_eq!(response["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(response["result"]["serverInfo"]["name"], "cflx-client");
        assert!(response["result"]["capabilities"]["tools"].is_object());
        assert!(session.is_initialized());

        // An unknown revision is answered with this server's own rather than
        // failing the session, which is what the handshake exists for.
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 2, "method": "initialize",
                "params": {"protocolVersion": "1999-01-01"}
            }),
        )
        .await;
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    /// A notification is absorbed silently — and `notifications/initialized` is
    /// an acknowledgement, never a substitute for the handshake it acknowledges.
    #[tokio::test]
    async fn a_notification_is_absorbed_without_any_response_frame() {
        let mut session = session();
        assert!(session
            .handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await
            .is_none());
        assert!(
            !session.is_initialized(),
            "a client cannot enable the tools by announcing itself initialized"
        );
        // A blank keepalive line is not a message either.
        assert!(session.handle("   ").await.is_none());

        let mut session = initialized(FakeHost::default()).await;
        // Idempotent: repeating it changes nothing and answers nothing.
        for _ in 0..2 {
            assert!(session
                .handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .await
                .is_none());
            assert!(session.is_initialized());
        }
    }

    /// The handshake is a gate, not a formality: a peer that skips it must not
    /// be able to reach a tool, and the refusal has to be machine-readable.
    #[tokio::test]
    async fn tools_are_refused_until_the_handshake_completes() {
        for method in ["tools/list", "tools/call"] {
            let mut session = Session::new(FakeHost::refusing("unreachable"));
            let response = respond(
                &mut session,
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": method,
                    "params": {"name": "cflx_status", "arguments": {}}
                }),
            )
            .await;
            assert_eq!(
                response["error"]["code"],
                rpc::SERVER_NOT_INITIALIZED,
                "{method} must be gated"
            );
            assert!(
                session.host.calls.lock().unwrap().is_empty(),
                "{method} must reach no owner before initialization"
            );
        }

        // `ping` is explicitly allowed before initialization.
        let mut session = session();
        let response = respond(
            &mut session,
            serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
        )
        .await;
        assert_eq!(response["result"], serde_json::json!({}));
        assert!(!session.is_initialized());
    }

    /// Every request envelope must identify JSON-RPC 2.0, and an invalid
    /// *notification* is still a notification: it gets no response at all.
    #[tokio::test]
    async fn a_frame_that_does_not_identify_json_rpc_2_is_an_invalid_request() {
        let mut session = initialized(FakeHost::refusing("unreachable")).await;
        for envelope in [
            serde_json::json!({"id": 1, "method": "tools/list"}),
            serde_json::json!({"jsonrpc": "1.0", "id": 1, "method": "tools/list"}),
            serde_json::json!({"jsonrpc": 2.0, "id": 1, "method": "tools/list"}),
        ] {
            let response = respond(&mut session, envelope.clone()).await;
            assert_eq!(
                response["error"]["code"],
                rpc::INVALID_REQUEST,
                "{envelope} must be refused"
            );
            assert_eq!(response["id"], 1, "a valid id is echoed even on refusal");
        }
        assert!(session.host.calls.lock().unwrap().is_empty());

        // No `jsonrpc` and no `id`: invalid, but still a notification.
        assert!(session
            .handle(r#"{"method":"notifications/initialized"}"#)
            .await
            .is_none());
    }

    /// Batch support is not advertised, and a malformed id cannot be echoed.
    #[tokio::test]
    async fn batches_and_unusable_ids_are_invalid_requests_answered_with_null() {
        let mut session = initialized(FakeHost::refusing("unreachable")).await;

        let frame = session
            .handle(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#)
            .await
            .expect("a batch owes one refusal");
        let response = parse(&frame);
        assert_eq!(response["error"]["code"], rpc::INVALID_REQUEST);
        assert!(response["id"].is_null());

        for unusable in [
            serde_json::json!({"jsonrpc": "2.0", "id": {"a": 1}, "method": "ping"}),
            serde_json::json!({"jsonrpc": "2.0", "id": [1], "method": "ping"}),
        ] {
            let frame = session
                .handle(&unusable.to_string())
                .await
                .expect("an invalid request object owes a refusal");
            let response = parse(&frame);
            assert_eq!(response["error"]["code"], rpc::INVALID_REQUEST);
            assert!(
                response["id"].is_null(),
                "an id this server cannot echo is answered with null"
            );
        }

        // A bare scalar is not a JSON-RPC message either.
        let frame = session.handle("42").await.expect("owed a refusal");
        assert_eq!(parse(&frame)["error"]["code"], rpc::INVALID_REQUEST);
        assert!(session.host.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ping_answers_an_empty_result() {
        let mut session = session();
        let response = respond(
            &mut session,
            serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}),
        )
        .await;
        assert_eq!(response["id"], 7);
        assert_eq!(response["result"], serde_json::json!({}));
    }

    #[tokio::test]
    async fn framing_faults_are_reported_as_json_rpc_errors_not_as_silence() {
        let mut session = session();
        let frame = session.handle("{not json").await.expect("owed a response");
        let response = parse(&frame);
        assert_eq!(response["error"]["code"], rpc::PARSE_ERROR);
        assert!(response["id"].is_null());

        let response = respond(
            &mut session,
            serde_json::json!({"jsonrpc": "2.0", "id": 3, "params": {}}),
        )
        .await;
        assert_eq!(response["error"]["code"], rpc::INVALID_REQUEST);

        let response = respond(
            &mut session,
            serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list"}),
        )
        .await;
        assert_eq!(response["error"]["code"], rpc::METHOD_NOT_FOUND);
    }

    /// The tool set is the security boundary, so it is asserted exactly rather
    /// than sampled: nothing here may name a command type, an expected revision,
    /// an idempotency key, an execution mark, or shell source.
    #[tokio::test]
    async fn tools_list_exposes_exactly_the_closed_intent_set() {
        let mut session = initialized(FakeHost::default()).await;
        let response = respond(
            &mut session,
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
        )
        .await;
        let tools = response["result"]["tools"].as_array().expect("a tool list");
        let names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, TOOL_NAMES.to_vec());

        let rendered = response.to_string();
        for forbidden in [
            "expected_revision",
            "idempotency_key",
            "set_execution_mark",
            "set_queue_intent",
            "command_type",
            "/api/v2/commands",
            // The retired admission surface, in every spelling it ever had: an
            // agent that could still name one would still be able to ask for the
            // analyze bypass this change removed.
            "cflx_enqueue",
            "cflx_wait",
            "cflx_notify",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "the closed tool surface must not mention {forbidden}"
            );
        }

        for tool in tools {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object");
            assert_eq!(
                schema["additionalProperties"], false,
                "{} must fail closed on unknown fields",
                tool["name"]
            );
            // Connection settings are on every tool, and the token is always a
            // variable *name*.
            assert!(schema["properties"]["auth_token_env"].is_object());
            assert!(
                schema["properties"].get("auth_token").is_none(),
                "a token value must never be an accepted argument"
            );
        }
    }

    #[tokio::test]
    async fn a_successful_call_returns_the_envelope_as_text_and_structured_content() {
        let envelope = ResultEnvelope::new(Operation::ControlMark, Outcome::Marked)
            .with_change("alpha")
            .with_instance(Some("i-1".to_string()));
        let mut session = initialized(FakeHost::answering(envelope)).await;
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {
                    "name": "cflx_control",
                    "arguments": {"action": "mark", "change_ids": ["alpha"]}
                }
            }),
        )
        .await;
        let result = &response["result"];
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["outcome"], "marked");
        assert_eq!(result["structuredContent"]["operation"], "control_mark");
        let text = result["content"][0]["text"].as_str().unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(reparsed["outcome"], "marked");
    }

    /// An unsuccessful *owner* outcome is a tool error the model can read, not a
    /// protocol error that would hide the reason from it.
    #[tokio::test]
    async fn an_unsuccessful_outcome_is_an_error_tool_result_carrying_its_reason() {
        let envelope = ResultEnvelope::new(Operation::ControlMark, Outcome::TargetIneligible)
            .with_change("alpha")
            .with_message("this owner refuses execution-mark mutation right now");
        let mut session = initialized(FakeHost::answering(envelope)).await;
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {
                    "name": "cflx_control",
                    "arguments": {"action": "mark", "change_ids": ["alpha"]}
                }
            }),
        )
        .await;
        assert!(response.get("error").is_none(), "the call itself succeeded");
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["structuredContent"]["outcome"],
            "target_ineligible"
        );
    }

    /// The three retired tools are gone from the dispatcher, not merely from the
    /// listing: a host that remembered a name from an older server must reach no
    /// owner with it.
    #[tokio::test]
    async fn the_retired_tools_are_unreachable_by_name() {
        for retired in [
            "cflx_enqueue",
            "cflx_wait",
            "cflx_notify_set",
            "cflx_notify_get",
            "cflx_notify_clear",
        ] {
            let mut session = initialized(FakeHost::refusing("unreachable")).await;
            let response = respond(
                &mut session,
                serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": {"name": retired, "arguments": {"change_id": "alpha"}}
                }),
            )
            .await;
            assert_eq!(response["result"]["isError"], true, "{retired}");
            assert!(
                session.host.calls.lock().unwrap().is_empty(),
                "{retired} must reach no owner"
            );
        }
    }

    #[tokio::test]
    async fn an_unknown_tool_is_refused_without_reaching_the_host() {
        let mut session = initialized(FakeHost::refusing("unreachable")).await;
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "cflx_force_stop", "arguments": {}}
            }),
        )
        .await;
        assert_eq!(response["result"]["isError"], true);
        assert!(session.host.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_host_refusal_becomes_an_error_result_rather_than_a_protocol_error() {
        let mut session =
            initialized(FakeHost::refusing("the tool arguments were not accepted")).await;
        let response = respond(
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "cflx_status", "arguments": {"nope": 1}}
            }),
        )
        .await;
        assert!(response.get("error").is_none());
        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not accepted"));
    }

    /// Every byte the server writes has to be a protocol frame, because an MCP
    /// host parses this stream and one stray line desynchronizes the session.
    #[tokio::test]
    async fn the_protocol_stream_carries_only_complete_json_rpc_frames() {
        let requests = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
            "not json at all\n",
        );
        let mut output = Vec::new();
        let status = serve(FakeHost::default(), requests.as_bytes(), &mut output).await;
        assert_eq!(status, 0);

        let text = String::from_utf8(output).expect("the stream is UTF-8");
        let frames: Vec<&str> = text.lines().collect();
        assert_eq!(
            frames.len(),
            3,
            "one frame per request with an id, and nothing for notifications or blank lines"
        );
        for frame in frames {
            let value: serde_json::Value =
                serde_json::from_str(frame).expect("every line is one JSON-RPC object");
            assert_eq!(value["jsonrpc"], "2.0");
            assert!(value.get("result").is_some() || value.get("error").is_some());
        }
    }

    /// A peer that never sends a newline must not be able to make this process
    /// hold its input. The bound is checked while bytes are read, so the frame
    /// never becomes a `String` at all, and the session ends unread rather than
    /// guessing where the next frame starts.
    #[tokio::test]
    async fn a_newline_free_oversized_frame_is_bounded_and_ends_the_session() {
        let mut input = Vec::with_capacity(MAX_FRAME_BYTES * 2);
        input.extend_from_slice(br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":"#);
        input.resize(MAX_FRAME_BYTES * 2, b'x');
        // Deliberately no trailing newline: the whole point is that waiting for
        // one is what an unbounded reader would do.

        let host = FakeHost::default();
        let mut output = Vec::new();
        let status = serve(host, input.as_slice(), &mut output).await;
        assert_eq!(status, Outcome::TransportError.exit_code());
        assert!(
            output.is_empty(),
            "an unread frame is answered with nothing, not with a guess"
        );
    }

    /// The bound is on retained bytes, not on how the peer chunks them: an
    /// oversized frame that *does* eventually terminate is refused just the same,
    /// and nothing after it is interpreted as another frame.
    #[tokio::test]
    async fn an_oversized_terminated_frame_dispatches_nothing_after_it() {
        let mut input = Vec::new();
        input.extend_from_slice(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        input.push(b'\n');
        let oversized = vec![b'x'; MAX_FRAME_BYTES + 1];
        input.extend_from_slice(&oversized);
        input.push(b'\n');
        input.extend_from_slice(
            br#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cflx_status"}}"#,
        );
        input.push(b'\n');

        let host = FakeHost::refusing("unreachable");
        let mut output = Vec::new();
        let status = serve(host, input.as_slice(), &mut output).await;
        assert_eq!(status, Outcome::TransportError.exit_code());
        let text = String::from_utf8(output).expect("the stream is UTF-8");
        assert_eq!(
            text.lines().count(),
            1,
            "only the initialize that preceded the oversized frame was answered: {text}"
        );
    }

    /// A frame that is not UTF-8 cannot be a JSON-RPC message, and resynchronizing
    /// past it would mean interpreting bytes of unknown provenance.
    #[tokio::test]
    async fn a_non_utf8_frame_ends_the_session_without_dispatching() {
        let input: Vec<u8> = vec![0xff, 0xfe, b'\n'];
        let mut output = Vec::new();
        let status = serve(FakeHost::default(), input.as_slice(), &mut output).await;
        assert_eq!(status, Outcome::TransportError.exit_code());
        assert!(output.is_empty());
    }

    /// Argument parsing fails closed: an unknown field is a refusal, never a
    /// silently-ignored instruction.
    #[test]
    fn tool_arguments_reject_unknown_fields() {
        let ok: Result<ControlArgs, _> =
            parse_args(&serde_json::json!({"action": "mark", "change_ids": ["alpha"]}));
        assert!(ok.is_ok());
        let extra: Result<ControlArgs, _> = parse_args(
            &serde_json::json!({"action": "mark", "change_ids": ["alpha"], "force": true}),
        );
        assert!(extra.is_err());
        let missing: Result<ControlArgs, _> = parse_args(&serde_json::json!({}));
        assert!(missing.is_err());
        // A lifecycle action names no targets, and the field defaults to empty
        // rather than being required, so `start` parses on its own.
        let lifecycle: Result<ControlArgs, _> = parse_args(&serde_json::json!({"action": "start"}));
        assert!(lifecycle.is_ok());
        // Subscription targets are always explicit: there is no list-all.
        let no_targets: Result<SubscribeArgs, _> =
            parse_args(&serde_json::json!({"action": "get"}));
        assert!(no_targets.is_err());
        // Absent arguments are an empty object, not a parse failure, for the one
        // tool that needs none.
        let none: Result<StatusArgs, _> = parse_args(&serde_json::Value::Null);
        assert!(none.is_ok());
    }

    /// The action vocabulary is the security boundary of `cflx_control`, so a
    /// name outside it must not resolve to anything at all.
    #[test]
    fn control_actions_are_a_closed_set() {
        use crate::client::control::Action;
        for name in ["mark", "unmark", "start", "stop", "force_stop"] {
            assert!(Action::parse(name).is_some(), "{name}");
        }
        for name in ["enqueue", "queue", "retry", "resolve", "archive", "merge"] {
            assert!(Action::parse(name).is_none(), "{name} must not be callable");
        }
    }

    #[test]
    fn a_change_id_argument_obeys_the_same_rule_as_the_cli() {
        assert_eq!(validated_change_id("alpha").unwrap(), "alpha");
        for rejected in ["../escape", "-leading", ".hidden", "with space", ""] {
            assert!(
                validated_change_id(rejected).is_err(),
                "{rejected} must be refused"
            );
        }
    }
}
