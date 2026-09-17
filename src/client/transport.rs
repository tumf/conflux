//! Bounded HTTP/1.1 over the owner's platform-local transport, and nothing else.
//!
//! `/api/v2` is served by axum over a plain byte stream, so speaking to it needs
//! a request line, a few headers, and a response parser — not a general HTTP
//! client dependency. Everything this module can do is deliberately visible in
//! one file: one request per connection with `Connection: close`, a hard byte
//! ceiling before deserialization, and no redirect, proxy, cookie, or retry
//! behavior at all.
//!
//! # One framing, two endpoints
//!
//! A Unix-domain socket and a Windows Named Pipe are both owner-only local byte
//! streams, and the bytes the client writes onto them are identical. So only
//! [`LocalApiClient::connect`] is platform-specific: request construction, token
//! validation, the timeout budget, the bounded read, HTTP parsing, chunk
//! decoding, event-marker detection, and error mapping are one implementation
//! over `AsyncRead + AsyncWrite`. Two copies of that would be two chances to
//! disagree about the response ceiling.
//!
//! Security posture:
//!
//! * the endpoint and every query value are data, never shell fragments;
//! * the bearer token is read from a named environment variable, is validated as
//!   an HTTP header value *before* a connection is opened, is sent only in the
//!   `Authorization` header, and is never placed in a message;
//! * a response is refused once it exceeds [`MAX_RESPONSE_BYTES`], before any
//!   parse, so a hostile or broken peer cannot make the client allocate without
//!   bound;
//! * on Windows the pipe namespace is machine-global, so deriving the right name
//!   proves nothing about who answered. The connected handle's owner SID is
//!   checked against this process's own before a single request byte is written.

#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Hard ceiling on one response, headers included.
///
/// Comfortably above any projection this API produces and far below anything
/// that would matter to a local client's memory.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// How long one request/response exchange may take.
///
/// A generous safety valve against a peer that accepts a connection and then
/// stops talking, not a latency budget: every observation loop has its own
/// deadline on top of this.
pub const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Where one owner's local `/api/v2` listener is.
///
/// A compile-time enum rather than a runtime one: a build has exactly one local
/// transport, and a variant the platform cannot connect to would be a branch
/// that exists only to be unreachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEndpoint {
    /// `${GIT_COMMON_DIR}/cflx-api.sock`, or an explicit `--unix-socket` path.
    #[cfg(unix)]
    Unix(PathBuf),
    /// `\\.\pipe\cflx-v2-<digest>` for this user and this repository.
    #[cfg(windows)]
    NamedPipe(String),
}

impl LocalEndpoint {
    /// How this endpoint is named in diagnostics and in `status` detail.
    ///
    /// Safe to print: a socket path is a filesystem location, and a pipe name is
    /// a digest that publishes neither the user's SID nor the repository path.
    pub fn label(&self) -> String {
        match self {
            #[cfg(unix)]
            Self::Unix(path) => path.display().to_string(),
            #[cfg(windows)]
            Self::NamedPipe(name) => name.clone(),
        }
    }

    /// The socket path, when this build's endpoint is a Unix socket.
    #[cfg(unix)]
    pub fn socket_path(&self) -> &Path {
        match self {
            Self::Unix(path) => path,
        }
    }
}

/// The connected stream type for this platform's local transport.
#[cfg(unix)]
type LocalStream = tokio::net::UnixStream;
#[cfg(windows)]
type LocalStream = tokio::net::windows::named_pipe::NamedPipeClient;

/// Why a request could not be completed.
///
/// Split by *what a caller should do*: `NotListening` means no owner,
/// `UntrustedPeer` means something answered and it is not this owner, `Timeout`
/// and `Io` mean the exchange failed, and `Protocol`/`TooLarge` mean the peer
/// answered something this client refuses to interpret.
#[derive(Debug)]
pub enum TransportError {
    /// Nothing is listening at the endpoint, or the entry is not one.
    NotListening { endpoint: String, detail: String },
    /// Something answered the endpoint, but it is not this user's owner.
    ///
    /// Never collapsed into `NotListening`: "no owner is running" invites a
    /// caller to start one, and "another principal owns this name" must not.
    /// Only the Named Pipe transport can raise it — a Unix socket's access
    /// control is the filesystem's, decided before `connect` returns.
    #[cfg_attr(not(windows), allow(dead_code))]
    UntrustedPeer(String),
    /// The exchange exceeded [`EXCHANGE_TIMEOUT`].
    Timeout,
    /// The connection failed mid-exchange.
    Io(String),
    /// The peer's bytes are not a response this client will parse.
    Protocol(String),
    /// The response exceeded [`MAX_RESPONSE_BYTES`].
    TooLarge,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotListening { endpoint, detail } => {
                write!(f, "no Conflux owner is listening at '{endpoint}': {detail}")
            }
            Self::UntrustedPeer(detail) => write!(f, "{detail}"),
            Self::Timeout => write!(f, "the owner did not answer within {EXCHANGE_TIMEOUT:?}"),
            Self::Io(detail) => write!(f, "the connection to the owner failed: {detail}"),
            Self::Protocol(detail) => write!(f, "the owner's response was not usable: {detail}"),
            Self::TooLarge => write!(
                f,
                "the owner's response exceeded the {MAX_RESPONSE_BYTES}-byte client limit"
            ),
        }
    }
}

impl std::error::Error for TransportError {}

/// One parsed HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Response body bytes.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Deserialize the body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, TransportError> {
        serde_json::from_slice(&self.body).map_err(|error| {
            TransportError::Protocol(format!("response body is not the expected JSON: {error}"))
        })
    }
}

/// Why a bearer token cannot be presented as an HTTP header value.
///
/// The value itself is never carried: the whole point of rejecting it is that
/// its bytes are hostile, and a diagnostic that echoed them would put a
/// credential — and a header injection someone attempted — into a log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRejection {
    /// A carriage return: the classic header-injection byte.
    CarriageReturn,
    /// A line feed: likewise.
    LineFeed,
    /// Any other C0 control byte.
    ControlCharacter,
    /// `DEL` (0x7F), which HTTP forbids in a field value.
    Delete,
}

impl TokenRejection {
    /// Stable, value-free description of the offending byte class.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CarriageReturn => "a carriage return (CR)",
            Self::LineFeed => "a line feed (LF)",
            Self::ControlCharacter => "an ASCII control character",
            Self::Delete => "the DEL character",
        }
    }
}

impl std::fmt::Display for TokenRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a resolved token may be sent as an `Authorization` header value.
///
/// A bearer token is opaque secret data to this client, but it is *not* opaque
/// to HTTP: a CR or LF inside it would terminate the header and let whatever
/// followed be read as another header or as the body. So the check happens once,
/// at the boundary where the value enters the process, and the request is never
/// built at all when it fails — "reject before connecting" is what makes the
/// injection unreachable rather than merely unlikely.
///
/// Everything from `0x20` up stays untouched, including non-ASCII `obs-text`
/// bytes, so a valid token is transmitted exactly as configured.
pub fn validate_token(value: &str) -> Result<(), TokenRejection> {
    for byte in value.bytes() {
        match byte {
            b'\r' => return Err(TokenRejection::CarriageReturn),
            b'\n' => return Err(TokenRejection::LineFeed),
            0x7F => return Err(TokenRejection::Delete),
            0x00..=0x1F => return Err(TokenRejection::ControlCharacter),
            _ => {}
        }
    }
    Ok(())
}

/// A client bound to one owner's local endpoint and one credential source.
#[derive(Clone)]
pub struct LocalApiClient {
    endpoint: LocalEndpoint,
    /// Bearer token, resolved once from the named environment variable.
    ///
    /// Held rather than re-read so a mid-run environment change cannot make two
    /// requests of one intent present different credentials.
    token: Option<String>,
}

/// Redacting `Debug`, written by hand rather than derived.
///
/// A derived one would print the bearer token, and "never emitted" has to hold
/// for a panic message and a stray `{:?}` too, not only for the paths someone
/// remembered to check.
impl std::fmt::Debug for LocalApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalApiClient")
            .field("endpoint", &self.endpoint.label())
            .field(
                "token",
                &if self.token.is_some() {
                    "<redacted>"
                } else {
                    "<none>"
                },
            )
            .finish()
    }
}

impl LocalApiClient {
    /// Bind a client to a local endpoint and an optional bearer token.
    ///
    /// Fallible because the token is validated here rather than at the write:
    /// a client that exists is a client whose credential can be framed, so no
    /// caller can reach [`Self::encode_request`] with a value that would inject
    /// a header.
    pub fn new(endpoint: LocalEndpoint, token: Option<String>) -> Result<Self, TokenRejection> {
        if let Some(token) = &token {
            validate_token(token)?;
        }
        Ok(Self { endpoint, token })
    }

    /// The endpoint this client talks to.
    pub fn endpoint(&self) -> &LocalEndpoint {
        &self.endpoint
    }

    /// Whether a bearer token will be presented.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// GET one resource.
    pub async fn get(&self, path_and_query: &str) -> Result<HttpResponse, TransportError> {
        self.request("GET", path_and_query, None).await
    }

    /// POST a JSON body.
    pub async fn post_json(
        &self,
        path_and_query: &str,
        body: &str,
    ) -> Result<HttpResponse, TransportError> {
        self.request("POST", path_and_query, Some(body)).await
    }

    /// PUT a JSON body.
    ///
    /// Used only by the execution-sink resources, which are idempotent
    /// replacements of one named subscription rather than submissions to a
    /// collection — the reason they are `PUT` and not `POST /commands`.
    pub async fn put_json(
        &self,
        path_and_query: &str,
        body: &str,
    ) -> Result<HttpResponse, TransportError> {
        self.request("PUT", path_and_query, Some(body)).await
    }

    /// DELETE one resource.
    pub async fn delete(&self, path_and_query: &str) -> Result<HttpResponse, TransportError> {
        self.request("DELETE", path_and_query, None).await
    }

    async fn request(
        &self,
        method: &str,
        path_and_query: &str,
        body: Option<&str>,
    ) -> Result<HttpResponse, TransportError> {
        let exchange = self.exchange(method, path_and_query, body);
        match tokio::time::timeout(EXCHANGE_TIMEOUT, exchange).await {
            Ok(result) => result,
            Err(_elapsed) => Err(TransportError::Timeout),
        }
    }

    async fn exchange(
        &self,
        method: &str,
        path_and_query: &str,
        body: Option<&str>,
    ) -> Result<HttpResponse, TransportError> {
        let mut stream = self.connect().await?;
        self.exchange_over(&mut stream, method, path_and_query, body)
            .await
    }

    /// Write one request and read one bounded response, over any byte stream.
    ///
    /// Generic on purpose: this is the whole wire behavior both platforms share,
    /// and making it independent of the connection type is what lets it be
    /// proven over an in-memory duplex rather than only against a live owner.
    async fn exchange_over<S>(
        &self,
        stream: &mut S,
        method: &str,
        path_and_query: &str,
        body: Option<&str>,
    ) -> Result<HttpResponse, TransportError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let request = self.encode_request(method, path_and_query, body);
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;

        // `Connection: close` makes the peer's EOF the end of the message, so
        // reading to EOF under a ceiling is both correct and bounded.
        let mut raw = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| TransportError::Io(error.to_string()))?;
            if read == 0 {
                break;
            }
            if raw.len() + read > MAX_RESPONSE_BYTES {
                return Err(TransportError::TooLarge);
            }
            raw.extend_from_slice(&chunk[..read]);
        }

        parse_response(&raw)
    }

    /// Open one connection to the owner's Unix socket.
    #[cfg(unix)]
    async fn connect(&self) -> Result<LocalStream, TransportError> {
        let path = self.endpoint.socket_path();
        tokio::net::UnixStream::connect(path)
            .await
            .map_err(|error| TransportError::NotListening {
                endpoint: path.display().to_string(),
                detail: error.to_string(),
            })
    }

    /// Open one connection to the owner's Named Pipe, and prove it is the owner.
    ///
    /// Two things happen here that have no Unix counterpart, because the pipe
    /// namespace is machine-global rather than filesystem-scoped:
    ///
    /// * the pipe is opened with identification-only security quality of
    ///   service, so a hostile server that did win the name cannot impersonate
    ///   this process beyond reading its identity;
    /// * the connected handle's owner SID is compared with this process's token
    ///   user SID *before* any request byte is written, so a credential and a
    ///   command never reach a peer that is not this user's owner.
    ///
    /// `ERROR_PIPE_BUSY` is the one retryable error: every instance is momentarily
    /// connected while the accept loop creates its replacement. The retry is
    /// bounded by the caller's own budget — [`EXCHANGE_TIMEOUT`] for a request,
    /// the caller's budget for an event stream — rather than by a second clock
    /// here, so no transport wait can outlive the operation that asked for it.
    #[cfg(windows)]
    async fn connect(&self) -> Result<LocalStream, TransportError> {
        use tokio::net::windows::named_pipe::ClientOptions;

        /// `SECURITY_IDENTIFICATION` from `winnt.h`: the server may learn who
        /// this client is and may not act as it. Tokio adds
        /// `SECURITY_SQOS_PRESENT` itself.
        const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
        /// The Win32 errors that mean "no owner here", as opposed to "an owner
        /// is here and something went wrong".
        const ERROR_FILE_NOT_FOUND: i32 = 2;
        const ERROR_PATH_NOT_FOUND: i32 = 3;
        const ERROR_ACCESS_DENIED: i32 = 5;
        const ERROR_PIPE_BUSY: i32 = 231;
        /// How long to wait before re-opening a momentarily busy pipe.
        const BUSY_RETRY: Duration = Duration::from_millis(20);

        let LocalEndpoint::NamedPipe(name) = &self.endpoint;
        loop {
            let opened = ClientOptions::new()
                .security_qos_flags(SECURITY_IDENTIFICATION)
                .open(name.as_str());
            return match opened {
                Ok(client) => {
                    self.verify_server_owner(&client)?;
                    Ok(client)
                }
                Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    tokio::time::sleep(BUSY_RETRY).await;
                    continue;
                }
                Err(error)
                    if matches!(
                        error.raw_os_error(),
                        Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_PATH_NOT_FOUND)
                    ) =>
                {
                    Err(TransportError::NotListening {
                        endpoint: name.clone(),
                        detail: error.to_string(),
                    })
                }
                Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
                    Err(TransportError::UntrustedPeer(format!(
                        "the Named Pipe '{name}' exists but refused this process: it belongs to \
                         another principal, so no request was sent"
                    )))
                }
                Err(error) => Err(TransportError::Io(error.to_string())),
            };
        }
    }

    /// Refuse a connected pipe whose owner is not this process's user.
    #[cfg(windows)]
    fn verify_server_owner(
        &self,
        client: &tokio::net::windows::named_pipe::NamedPipeClient,
    ) -> Result<(), TransportError> {
        let expected = crate::web::named_pipe::current_user_sid().map_err(|error| {
            TransportError::UntrustedPeer(format!(
                "the connected Named Pipe owner could not be checked against this process: {error}"
            ))
        })?;
        let observed = crate::web::named_pipe::handle_owner_sid(client).map_err(|error| {
            TransportError::UntrustedPeer(format!(
                "the connected Named Pipe's owner could not be read, so it was not trusted: \
                 {error}"
            ))
        })?;
        if expected != observed {
            return Err(TransportError::UntrustedPeer(format!(
                "the Named Pipe '{}' is owned by '{observed}', not by this process's user \
                 '{expected}'. No request was sent, and no other transport was tried",
                self.endpoint.label()
            )));
        }
        Ok(())
    }

    /// Build the request bytes.
    ///
    /// Separate from the I/O so the exact wire form — including the fact that
    /// the token appears only in `Authorization` — is assertable without a
    /// connection.
    pub fn encode_request(&self, method: &str, path_and_query: &str, body: Option<&str>) -> String {
        let mut request = format!(
            "{method} {path_and_query} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Accept: application/json\r\n\
             User-Agent: cflx-client/{}\r\n\
             Connection: close\r\n",
            env!("CARGO_PKG_VERSION")
        );
        if let Some(token) = &self.token {
            request.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        match body {
            Some(body) => {
                request.push_str("Content-Type: application/json\r\n");
                request.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
                request.push_str(body);
            }
            None => request.push_str("\r\n"),
        }
        request
    }
}

/// Parse a complete HTTP/1.1 response.
///
/// Handles the two framings axum actually produces — `Content-Length` and
/// `Transfer-Encoding: chunked` — and treats anything else as a protocol error
/// rather than guessing where the body ends.
pub fn parse_response(raw: &[u8]) -> Result<HttpResponse, TransportError> {
    if raw.is_empty() {
        return Err(TransportError::Protocol(
            "the owner closed the connection without answering".to_string(),
        ));
    }
    let separator = find_subslice(raw, b"\r\n\r\n").ok_or_else(|| {
        TransportError::Protocol("the response has no complete header block".to_string())
    })?;
    let head = std::str::from_utf8(&raw[..separator])
        .map_err(|_| TransportError::Protocol("the response head is not UTF-8".to_string()))?;
    let mut lines = head.split("\r\n");

    let status_line = lines
        .next()
        .ok_or_else(|| TransportError::Protocol("the response has no status line".to_string()))?;
    let status = parse_status(status_line)?;

    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "content-length" => {
                content_length = Some(value.parse().map_err(|_| {
                    TransportError::Protocol(format!("invalid Content-Length '{value}'"))
                })?);
            }
            "transfer-encoding" if value.eq_ignore_ascii_case("chunked") => chunked = true,
            _ => {}
        }
    }

    let rest = &raw[separator + 4..];
    let body = if chunked {
        decode_chunked(rest)?
    } else if let Some(length) = content_length {
        if rest.len() < length {
            return Err(TransportError::Protocol(
                "the response body is shorter than its Content-Length".to_string(),
            ));
        }
        rest[..length].to_vec()
    } else {
        // No framing header: `Connection: close` means the body runs to EOF,
        // which is exactly what was already read.
        rest.to_vec()
    };

    if body.len() > MAX_RESPONSE_BYTES {
        return Err(TransportError::TooLarge);
    }
    Ok(HttpResponse { status, body })
}

fn parse_status(status_line: &str) -> Result<u16, TransportError> {
    let mut parts = status_line.split(' ');
    let version = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") {
        return Err(TransportError::Protocol(format!(
            "unexpected status line '{status_line}'"
        )));
    }
    parts
        .next()
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| TransportError::Protocol(format!("unexpected status line '{status_line}'")))
}

fn decode_chunked(mut rest: &[u8]) -> Result<Vec<u8>, TransportError> {
    let mut body = Vec::new();
    loop {
        let line_end = find_subslice(rest, b"\r\n").ok_or_else(|| {
            TransportError::Protocol("a chunk header is not terminated".to_string())
        })?;
        let header = std::str::from_utf8(&rest[..line_end])
            .map_err(|_| TransportError::Protocol("a chunk header is not UTF-8".to_string()))?;
        // A chunk extension after ';' is legal and ignorable.
        let size_text = header.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| TransportError::Protocol(format!("invalid chunk size '{size_text}'")))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            return Ok(body);
        }
        if body.len() + size > MAX_RESPONSE_BYTES {
            return Err(TransportError::TooLarge);
        }
        if rest.len() < size + 2 {
            return Err(TransportError::Protocol(
                "a chunk is shorter than its declared size".to_string(),
            ));
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Why an observation loop woke up.
///
/// This is a *hint*, never evidence. Whatever it says, the caller re-observes
/// the authoritative resources before deciding anything, so a missed frame only
/// costs one poll interval and a spurious one only costs one extra read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    /// The owner published something state-affecting.
    Activity,
    /// The cursor is no longer replayable; rehydrate everything.
    Gap,
    /// Nothing arrived before the caller's budget expired.
    Idle,
}

/// Sentinels that identify a state-affecting or gap envelope on the stream.
///
/// Substring detection rather than incremental SSE-plus-chunked parsing,
/// because the only thing this needs to decide is "re-observe now or at the
/// next poll". Building a second, weaker event decoder to answer a question the
/// authoritative reads answer anyway would be more code and more ways to be
/// wrong.
const STATE_MARKER: &str = "\"category\":\"state\"";
const GAP_MARKER: &str = "\"category\":\"gap\"";

impl LocalApiClient {
    /// Wait for the owner to publish activity, up to `budget`.
    ///
    /// Opens the ordered event stream at the caller's cursor and closes it as
    /// soon as it has something to report. Any transport problem is reported as
    /// [`Wake::Idle`]: the stream is an optimization, and a client that failed
    /// its wait because a keep-alive frame was malformed would be worse than one
    /// that simply polled.
    pub async fn wake_on_activity(
        &self,
        after_sequence: u64,
        instance_id: &str,
        budget: Duration,
    ) -> Wake {
        match tokio::time::timeout(
            budget,
            self.read_stream_until_activity(after_sequence, instance_id),
        )
        .await
        {
            Ok(Ok(wake)) => wake,
            Ok(Err(_)) | Err(_) => Wake::Idle,
        }
    }

    async fn read_stream_until_activity(
        &self,
        after_sequence: u64,
        instance_id: &str,
    ) -> Result<Wake, TransportError> {
        let path = format!(
            "/api/v2/events?after_sequence={after_sequence}&instance_id={}",
            encode_query_value(instance_id)
        );
        let mut stream = self.connect().await?;
        self.watch_stream(&mut stream, &path).await
    }

    /// Read the event stream until a marker appears, over any byte stream.
    ///
    /// Shared with the request path for the same reason the exchange is: the
    /// wake markers, the retained-tail bound, and the "EOF means idle" rule are
    /// transport-independent, and a second copy for Windows would be a second
    /// place for them to drift.
    async fn watch_stream<S>(
        &self,
        stream: &mut S,
        path_and_query: &str,
    ) -> Result<Wake, TransportError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let request = self.encode_request("GET", path_and_query, None);
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;

        let mut seen = String::new();
        let mut chunk = [0u8; 8 * 1024];
        loop {
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| TransportError::Io(error.to_string()))?;
            if read == 0 {
                return Ok(Wake::Idle);
            }
            seen.push_str(&String::from_utf8_lossy(&chunk[..read]));
            if seen.contains(GAP_MARKER) {
                return Ok(Wake::Gap);
            }
            if seen.contains(STATE_MARKER) {
                return Ok(Wake::Activity);
            }
            // Keep only enough tail to span a marker split across two reads.
            if seen.len() > 64 * 1024 {
                let tail = seen.split_off(seen.len() - 128);
                seen = tail;
            }
        }
    }
}

/// Percent-encode one query value.
///
/// Change IDs are already validated at parse time, but the encoder is applied
/// anyway: a validator and an encoder answer different questions, and only the
/// encoder makes the request unambiguous.
pub fn encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An endpoint of whichever kind this build actually connects to.
    ///
    /// The assertions below are about request framing and response parsing,
    /// which are the same on both platforms; only the label differs.
    fn endpoint() -> LocalEndpoint {
        #[cfg(unix)]
        {
            LocalEndpoint::Unix(PathBuf::from("/tmp/cflx.sock"))
        }
        #[cfg(windows)]
        {
            LocalEndpoint::NamedPipe(r"\\.\pipe\cflx-v2-test".to_string())
        }
    }

    fn client() -> LocalApiClient {
        LocalApiClient::new(endpoint(), Some("s3cret".to_string()))
            .expect("a plain token is a valid header value")
    }

    #[test]
    fn a_token_travels_only_in_the_authorization_header() {
        let request = client().encode_request("GET", "/api/v2/state", None);
        assert!(request.contains("Authorization: Bearer s3cret\r\n"));
        let (head, _) = request.split_once("\r\n").unwrap();
        assert_eq!(head, "GET /api/v2/state HTTP/1.1");
        // The request line and every other header must be credential free.
        let without_auth: String = request
            .lines()
            .filter(|line| !line.starts_with("Authorization:"))
            .collect();
        assert!(!without_auth.contains("s3cret"), "{without_auth}");
    }

    #[test]
    fn debugging_a_client_never_renders_its_token() {
        let rendered = format!("{:?}", client());
        assert!(!rendered.contains("s3cret"), "{rendered}");
        assert!(rendered.contains("<redacted>"), "{rendered}");
        let anonymous = format!(
            "{:?}",
            LocalApiClient::new(endpoint(), None).expect("no token")
        );
        assert!(anonymous.contains("<none>"), "{anonymous}");
    }

    #[test]
    fn a_tokenless_client_sends_no_authorization_header() {
        let anonymous = LocalApiClient::new(endpoint(), None).expect("no token");
        let request = anonymous.encode_request("GET", "/api/v2/health", None);
        assert!(!request.contains("Authorization"));
        assert!(!anonymous.has_token());
    }

    #[test]
    fn a_body_carries_its_own_length_and_content_type() {
        let request = client().encode_request("POST", "/api/v2/commands", Some(r#"{"a":1}"#));
        assert!(request.contains("Content-Type: application/json\r\n"));
        assert!(request.contains("Content-Length: 7\r\n"));
        assert!(request.ends_with("\r\n\r\n{\"a\":1}"));
    }

    #[test]
    fn an_endpoint_labels_itself_without_disclosing_a_credential() {
        let label = endpoint().label();
        assert!(!label.is_empty());
        assert!(!label.contains("s3cret"));
    }

    #[test]
    fn a_content_length_response_is_parsed_exactly() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 7\r\n\r\n{\"a\":1}trailing";
        let response = parse_response(raw).expect("parses");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"{\"a\":1}");
    }

    #[test]
    fn a_chunked_response_is_reassembled() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n";
        let response = parse_response(raw).expect("parses");
        assert_eq!(response.body, b"{\"a\":1}");
    }

    #[test]
    fn a_bodyless_close_delimited_response_is_accepted() {
        let raw = b"HTTP/1.1 204 No Content\r\nCache-Control: no-store\r\n\r\n";
        let response = parse_response(raw).expect("parses");
        assert_eq!(response.status, 204);
        assert!(response.body.is_empty());
    }

    #[test]
    fn a_truncated_or_nonsense_response_is_a_protocol_error_not_a_guess() {
        for raw in [
            &b""[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nshort"[..],
            &b"GARBAGE\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: nine\r\n\r\nshort"[..],
        ] {
            assert!(
                matches!(parse_response(raw), Err(TransportError::Protocol(_))),
                "raw={:?}",
                String::from_utf8_lossy(raw)
            );
        }
    }

    #[test]
    fn an_oversized_body_is_refused_before_it_is_interpreted() {
        let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let mut raw =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
        raw.extend_from_slice(&body);
        assert!(matches!(
            parse_response(&raw),
            Err(TransportError::TooLarge)
        ));
    }

    #[test]
    fn query_values_are_encoded_rather_than_interpolated() {
        assert_eq!(encode_query_value("add-client-cli"), "add-client-cli");
        assert_eq!(encode_query_value("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(encode_query_value("../etc"), "..%2Fetc");
    }

    // ------------------------------------------------------------------
    // The shared framing, proven without a socket or a pipe.
    //
    // `exchange_over` and `watch_stream` are the whole wire behavior both
    // platforms share, so an in-memory duplex is the honest unit boundary:
    // what these assert is that one stream of bytes in produces one bounded
    // result out, which is exactly what must not vary by transport.
    // ------------------------------------------------------------------

    /// Serve `response` to whatever is written, and return what was written.
    async fn round_trip(
        client: &LocalApiClient,
        method: &str,
        path: &str,
        body: Option<&str>,
        response: &'static [u8],
    ) -> (String, Result<HttpResponse, TransportError>) {
        let (mut ours, mut theirs) = tokio::io::duplex(8 * 1024);
        let peer = tokio::spawn(async move {
            let mut request = Vec::new();
            // The client writes its whole request, then waits, so reading a
            // bounded amount and answering is enough to complete the exchange.
            let mut chunk = [0u8; 4096];
            let read = theirs.read(&mut chunk).await.expect("request bytes");
            request.extend_from_slice(&chunk[..read]);
            theirs.write_all(response).await.expect("response bytes");
            theirs
                .shutdown()
                .await
                .expect("close like Connection: close");
            String::from_utf8(request).expect("UTF-8 request")
        });
        let result = client.exchange_over(&mut ours, method, path, body).await;
        (peer.await.expect("peer task"), result)
    }

    #[tokio::test]
    async fn one_framing_serves_any_local_byte_stream() {
        let (request, result) = round_trip(
            &client(),
            "POST",
            "/api/v2/commands",
            Some(r#"{"a":1}"#),
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n{\"b\"",
        )
        .await;

        assert!(
            request.starts_with("POST /api/v2/commands HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(request.contains("Connection: close\r\n"), "{request}");
        assert!(
            request.contains("Authorization: Bearer s3cret\r\n"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"a":1}"#), "{request}");

        let response = result.expect("the duplex peer answered");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"{\"b\"");
    }

    #[tokio::test]
    async fn the_response_ceiling_applies_to_every_local_transport() {
        // Declared far above the ceiling and delivered incrementally: the
        // refusal has to happen while reading, not after allocating.
        static HUGE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n";
        let (mut ours, mut theirs) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let mut chunk = [0u8; 4096];
            let _ = theirs.read(&mut chunk).await;
            theirs.write_all(HUGE).await.expect("head");
            let filler = vec![b'x'; 64 * 1024];
            // More than the ceiling, written until the client gives up.
            for _ in 0..(MAX_RESPONSE_BYTES / filler.len() + 2) {
                if theirs.write_all(&filler).await.is_err() {
                    break;
                }
            }
        });
        let result = client()
            .exchange_over(&mut ours, "GET", "/api/v2/state", None)
            .await;
        assert!(
            matches!(result, Err(TransportError::TooLarge)),
            "{result:?}"
        );
        peer.abort();
    }

    #[tokio::test]
    async fn the_event_stream_wakes_on_the_same_markers_over_any_stream() {
        for (frame, expected) in [
            (
                &b"HTTP/1.1 200 OK\r\n\r\ndata: {\"category\":\"state\"}\n\n"[..],
                Wake::Activity,
            ),
            (
                &b"HTTP/1.1 200 OK\r\n\r\ndata: {\"category\":\"gap\"}\n\n"[..],
                Wake::Gap,
            ),
            (&b"HTTP/1.1 200 OK\r\n\r\n: keep-alive\n\n"[..], Wake::Idle),
        ] {
            let (mut ours, mut theirs) = tokio::io::duplex(8 * 1024);
            let frame = frame.to_vec();
            let peer = tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                let _ = theirs.read(&mut chunk).await;
                let _ = theirs.write_all(&frame).await;
                let _ = theirs.shutdown().await;
            });
            let wake = client()
                .watch_stream(&mut ours, "/api/v2/events?after_sequence=0")
                .await
                .expect("the duplex peer answered");
            assert_eq!(wake, expected);
            let _ = peer.await;
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_absent_socket_reports_no_owner_rather_than_an_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let client =
            LocalApiClient::new(LocalEndpoint::Unix(tmp.path().join("missing.sock")), None)
                .expect("no token");
        let error = client.get("/api/v2/health").await.expect_err("no owner");
        assert!(matches!(error, TransportError::NotListening { .. }));
        assert!(error.to_string().contains("no Conflux owner is listening"));
    }

    /// An absent pipe is "no owner", exactly as an absent socket is.
    #[cfg(windows)]
    #[tokio::test]
    async fn an_absent_pipe_reports_no_owner_rather_than_an_io_error() {
        let client = LocalApiClient::new(
            LocalEndpoint::NamedPipe(r"\\.\pipe\cflx-v2-nothing-is-here".to_string()),
            None,
        )
        .expect("no token");
        let error = client.get("/api/v2/health").await.expect_err("no owner");
        assert!(
            matches!(error, TransportError::NotListening { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("no Conflux owner is listening"));
    }
}
