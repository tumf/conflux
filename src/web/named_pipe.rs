//! Repository-scoped Windows Named Pipe listener for the local `/api/v2` server.
//!
//! Windows has no Unix-domain socket under the Git common directory to hang the
//! local API on, and the pipe namespace is machine-global rather than
//! filesystem-scoped. So the endpoint identity that `cflx-api.sock` gets for free
//! from its *location* has to be constructed here: the pipe name is a digest of
//! the current user's SID and the canonical Git common directory, and the pipe's
//! DACL grants access to that user alone.
//!
//! Two properties matter and are tested separately:
//!
//! * **Determinism.** Every linked worktree of one repository derives one name,
//!   exactly as every linked worktree resolves one socket path, because both
//!   derive from the same canonical common directory the repository lock
//!   excludes on. Distinct repositories, and distinct local users, diverge.
//! * **Non-disclosure.** Neither the SID nor the repository path survives into
//!   the published name. A machine-global namespace is readable by anything that
//!   can enumerate `\\.\pipe\`, so the name is a digest and never a rendering.
//!
//! Nothing here is durable workflow state: the pipe exists for the lifetime of
//! one owner process, and the next action for a workspace never depends on
//! whether it exists.

// On a host that binds no pipe, the derivation below is reachable only from its
// own tests. It stays compiled there deliberately — the identity rules are what
// decide whether a client reaches its own owner, and a rule proven only on the
// platform that already works is not proven — so "unused here" is the expected
// state rather than a finding.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::Path;

use sha2::{Digest, Sha256};

/// Fixed prefix of every default pipe this build creates.
///
/// The `v2` is the API contract's, not a name-format version: a build that
/// spoke a different `/api/v2` would still be reachable on this name, and the
/// client's own version check is what refuses it.
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\cflx-v2-";

/// Normalize a canonical Git common directory into digest input.
///
/// Windows path equality is not string equality, so two spellings of one
/// repository must reduce to one digest input before hashing or the main
/// worktree and a linked worktree would derive different endpoints:
///
/// 1. `std::fs::canonicalize` returns an extended-length `\\?\` path; the prefix
///    is removed, and `\\?\UNC\server\share` becomes `\\server\share`.
/// 2. Forward slashes — which Git happily emits — become backslashes.
/// 3. The result is lowercased once, which is how the case-insensitive
///    comparison Windows performs is represented in a digest that has no notion
///    of case at all.
///
/// Textual rather than filesystem work on purpose: the caller has already
/// canonicalized through the repository-discovery path, and re-canonicalizing
/// here would make the derivation depend on whether the directory still exists.
pub fn normalize_common_dir(common_dir: &Path) -> String {
    let raw = common_dir.to_string_lossy().replace('/', "\\");
    let stripped = match raw.strip_prefix(r"\\?\") {
        // `\\?\UNC\server\share` is the extended spelling of `\\server\share`;
        // collapsing it keeps a UNC repository from deriving two endpoints.
        Some(rest) => match rest.strip_prefix(r"UNC\") {
            Some(unc) => format!(r"\\{unc}"),
            None => rest.to_string(),
        },
        None => raw.clone(),
    };
    stripped.to_lowercase()
}

/// The default pipe name for one user and one canonical Git common directory.
///
/// The digest input is `UTF-8(sid) || NUL || UTF-8(normalized common dir)`. The
/// NUL is a separator rather than decoration: without it a SID ending in `-1`
/// and a path starting with `c:\…` could be re-cut into a different pair that
/// hashed the same, which would let one user's endpoint collide with another's.
pub fn pipe_name_for(user_sid: &str, common_dir: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(user_sid.as_bytes());
    digest.update([0u8]);
    digest.update(normalize_common_dir(common_dir).as_bytes());
    let mut name = String::with_capacity(PIPE_NAME_PREFIX.len() + 64);
    name.push_str(PIPE_NAME_PREFIX);
    for byte in digest.finalize() {
        name.push_str(&format!("{byte:02x}"));
    }
    name
}

/// The `pipe://` form used in endpoint metadata and diagnostics.
///
/// Discovery information for local clients, exactly as `unix://` is: no browser
/// can open it, and the QR popup still encodes the TCP URL only.
pub fn pipe_endpoint(name: &str) -> String {
    format!("pipe://{name}")
}

// `OwnerOnlySecurity` is re-exported for the native test suite, which builds a
// deliberately unopenable endpoint through the same creation path the owner
// uses. The binary itself only ever reaches it through `NamedPipeAcceptor`.
#[cfg(windows)]
#[allow(unused_imports)]
pub use windows_impl::{
    current_user_sid, default_pipe_name, handle_owner_sid, NamedPipeAcceptor, OwnerOnlySecurity,
};

/// Everything that needs a real Windows token, DACL, or pipe handle.
///
/// Split out so the identity rules above stay testable on any host: the
/// derivation is the part that decides whether two worktrees agree, and proving
/// it should not require a Windows builder.
#[cfg(windows)]
mod windows_impl {
    use std::ffi::c_void;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::sync::Arc;

    use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};
    use tokio_util::sync::CancellationToken;
    use tracing::{debug, error};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
        SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// `ERROR_SUCCESS`, the only `WIN32_ERROR` that means the call worked.
    const ERROR_SUCCESS: u32 = 0;

    /// Input and output buffer hint for every instance.
    ///
    /// A hint to the kernel's allocation, not a limit on the exchange: the
    /// client's own [`MAX_RESPONSE_BYTES`] ceiling is what bounds a response.
    ///
    /// [`MAX_RESPONSE_BYTES`]: crate::client::transport::MAX_RESPONSE_BYTES
    const PIPE_BUFFER_BYTES: u32 = 64 * 1024;

    /// How long the accept loop pauses after a failed wait for a client.
    ///
    /// Short enough to be invisible to an operator, long enough that a handle
    /// that keeps failing cannot spin the runtime.
    const ACCEPT_ERROR_BACKOFF: std::time::Duration = std::time::Duration::from_millis(20);

    /// The default pipe name for this user and this repository.
    pub fn default_pipe_name(common_dir: &Path) -> Result<String, String> {
        Ok(super::pipe_name_for(&current_user_sid()?, common_dir))
    }

    /// The current process token's `TokenUser` SID, in canonical `S-1-…` form.
    ///
    /// The string form is what both consumers need: SDDL takes a string SID, and
    /// the client's pre-request owner check compares canonical strings rather
    /// than juggling two borrowed `PSID` lifetimes to reach the same verdict.
    pub fn current_user_sid() -> Result<String, String> {
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|error| format!("failed to open this process's access token: {error}"))?;
            let token = OwnedHandle(token);

            let mut needed = 0u32;
            // The first call is expected to fail with "buffer too small"; it is
            // asked only for the size, which is why its error is discarded.
            let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
            if needed == 0 {
                return Err("the access token reported no TokenUser size".to_string());
            }
            let mut buffer = vec![0u8; needed as usize];
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(buffer.as_mut_ptr().cast::<c_void>()),
                needed,
                &mut needed,
            )
            .map_err(|error| format!("failed to read this process's user SID: {error}"))?;

            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            sid_to_string(user.User.Sid)
        }
    }

    /// Render a `PSID` as its canonical string form.
    ///
    /// # Safety
    ///
    /// `sid` must point at a valid SID that outlives this call.
    unsafe fn sid_to_string(sid: PSID) -> Result<String, String> {
        let mut raw = PWSTR::null();
        ConvertSidToStringSidW(sid, &mut raw)
            .map_err(|error| format!("failed to render a SID: {error}"))?;
        if raw.is_null() {
            return Err("converting a SID produced no string".to_string());
        }
        let rendered = raw.to_string();
        // Windows allocated this with `LocalAlloc`, so it is freed here rather
        // than by the string that was copied out of it.
        let _ = LocalFree(HLOCAL(raw.as_ptr().cast::<c_void>()));
        rendered.map_err(|error| format!("a SID string was not valid UTF-16: {error}"))
    }

    /// A process handle that closes itself.
    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    /// A self-relative security descriptor granting the current user, and only
    /// the current user, full access.
    ///
    /// The SDDL is `D:P(A;;GA;;;<sid>)`. `P` makes it protected, so nothing is
    /// inherited from the namespace's default, and the single ACE grants
    /// `GENERIC_ALL` to one principal — deliberately not to Administrators or
    /// SYSTEM, because an owner-only endpoint that a second principal can open
    /// is not owner-only.
    ///
    /// Held rather than rebuilt per instance so the descriptor outlives every
    /// `CreateNamedPipeW` call that reads it, and freed exactly once on drop.
    pub struct OwnerOnlySecurity {
        descriptor: PSECURITY_DESCRIPTOR,
    }

    // The descriptor is an immutable self-relative blob in `LocalAlloc` memory.
    // Nothing mutates it after construction, and the only thread-affine thing
    // about it — the allocation — is freed by whichever thread drops it last.
    unsafe impl Send for OwnerOnlySecurity {}
    unsafe impl Sync for OwnerOnlySecurity {}

    impl OwnerOnlySecurity {
        /// Build the descriptor for the current process's user.
        pub fn for_current_user() -> Result<Self, String> {
            Self::for_sid(&current_user_sid()?)
        }

        /// Build the descriptor for one canonical string SID.
        ///
        /// The `O:` component is not decoration. Windows stamps a new object's
        /// owner from the creating token's *`TokenOwner`*, which for an elevated
        /// or administrator account is `BUILTIN\Administrators`
        /// (`S-1-5-32-544`) rather than the user — so a pipe created without an
        /// explicit owner would be owned by a SID the client's pre-request check
        /// cannot distinguish from any other elevated process's. Naming the owner
        /// here makes "this endpoint is mine" mean the same thing for an elevated
        /// owner as for a standard one, and a process may always set an object's
        /// owner to its own user SID.
        pub fn for_sid(sid: &str) -> Result<Self, String> {
            Self::from_sddl(&format!("O:{sid}D:P(A;;GA;;;{sid})"))
        }

        /// Build a descriptor from an explicit SDDL string.
        ///
        /// The owner's own pipes always come from [`Self::for_current_user`].
        /// This exists so a native test can create an endpoint through the
        /// *production* creation path that this user may deliberately not open —
        /// the only honest way to prove the client refuses such a pipe instead
        /// of degrading to another transport.
        pub fn from_sddl(sddl: &str) -> Result<Self, String> {
            let sddl: Vec<u16> = format!("{sddl}\0").encode_utf16().collect();
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(sddl.as_ptr()),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    None,
                )
                .map_err(|error| {
                    format!("failed to build the owner-only pipe security descriptor: {error}")
                })?;
            }
            if descriptor.is_invalid() {
                return Err("the owner-only pipe security descriptor was not created".to_string());
            }
            Ok(Self { descriptor })
        }

        /// Create one server instance under this descriptor.
        ///
        /// `first` requests exclusive first-instance ownership, which is what
        /// turns "somebody already owns this repository's endpoint" into a hard
        /// startup refusal instead of a silent second owner.
        pub fn create_instance(&self, name: &str, first: bool) -> io::Result<NamedPipeServer> {
            let mut attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.descriptor.0,
                bInheritHandle: false.into(),
            };
            let mut options = ServerOptions::new();
            options
                .first_pipe_instance(first)
                // Byte mode: the wire is HTTP/1.1, and message framing would cut
                // a response at an instance boundary the parser knows nothing of.
                .pipe_mode(PipeMode::Byte)
                // The whole point of a local endpoint. A pipe that accepted a
                // remote client would be reachable from the network even though
                // nothing published a port.
                .reject_remote_clients(true)
                .in_buffer_size(PIPE_BUFFER_BYTES)
                .out_buffer_size(PIPE_BUFFER_BYTES);
            // Safety: `attributes` is a fully initialized SECURITY_ATTRIBUTES
            // whose descriptor is owned by `self` and outlives this call.
            unsafe {
                options.create_with_security_attributes_raw(
                    name,
                    std::ptr::addr_of_mut!(attributes).cast::<c_void>(),
                )
            }
        }
    }

    impl Drop for OwnerOnlySecurity {
        fn drop(&mut self) {
            if !self.descriptor.is_invalid() {
                unsafe {
                    let _ = LocalFree(HLOCAL(self.descriptor.0));
                }
            }
        }
    }

    /// The owner's accept loop, shaped as something `axum::serve` can drive.
    ///
    /// A Named Pipe has no listener object: each instance *is* a connection once
    /// a client arrives. So "accepting" means holding one unconnected instance,
    /// awaiting a client on it, and creating the replacement **before** the
    /// connected one is handed to the router — otherwise the window between
    /// dispatch and replacement is a window in which the endpoint does not exist
    /// and a client sees `owner_not_running` for a live owner.
    pub struct NamedPipeAcceptor {
        name: String,
        security: Arc<OwnerOnlySecurity>,
        /// The unconnected instance a client may arrive on, or `None` once the
        /// listener has given up.
        pending: Option<NamedPipeServer>,
        /// The whole listener plan's token: a replacement that cannot be created
        /// takes the owner down rather than leaving orchestration alive without
        /// its local control endpoint.
        shutdown: CancellationToken,
    }

    impl NamedPipeAcceptor {
        /// Acquire the first instance of `name`, or fail startup.
        pub fn bind(name: &str, shutdown: CancellationToken) -> Result<Self, String> {
            let security = Arc::new(OwnerOnlySecurity::for_current_user()?);
            let pending = security.create_instance(name, true).map_err(|error| {
                format!(
                    "failed to create the first instance of Named Pipe '{name}': {error}. \
                     Another Conflux owner may already hold this repository's endpoint"
                )
            })?;
            Ok(Self {
                name: name.to_string(),
                security,
                pending: Some(pending),
                shutdown,
            })
        }
    }

    impl axum::serve::Listener for NamedPipeAcceptor {
        type Io = NamedPipeServer;
        type Addr = String;

        async fn accept(&mut self) -> (Self::Io, Self::Addr) {
            loop {
                let Some(server) = self.pending.take() else {
                    // The listener already gave up and cancelled the plan.
                    // Parking is the only honest answer: `accept` cannot report
                    // an error, and returning a connection there is none of
                    // would be worse than never returning.
                    std::future::pending::<()>().await;
                    unreachable!("a cancelled acceptor never resumes");
                };
                match server.connect().await {
                    Ok(()) => {}
                    Err(error) => {
                        // A failed wait leaves the instance usable, so this is a
                        // retry rather than a loss of the endpoint. The pause is
                        // what keeps a persistently failing handle from becoming
                        // a busy loop that starves the runtime, which is the same
                        // reason axum's own TCP listener backs off.
                        debug!("Named Pipe connect failed: {error}");
                        self.pending = Some(server);
                        tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                        continue;
                    }
                }
                match self.security.create_instance(&self.name, false) {
                    Ok(next) => {
                        self.pending = Some(next);
                        return (server, self.name.clone());
                    }
                    Err(error) => {
                        // The connected client is dropped unserved on purpose:
                        // serving it would advertise an endpoint that has just
                        // stopped existing for everybody else.
                        drop(server);
                        error!(
                            "failed to create a replacement instance of Named Pipe '{}': {error}. \
                             Stopping the local API and shutting down.",
                            self.name
                        );
                        self.shutdown.cancel();
                    }
                }
            }
        }

        fn local_addr(&self) -> io::Result<Self::Addr> {
            Ok(self.name.clone())
        }
    }

    /// The owner SID of an already-connected pipe handle.
    ///
    /// The pipe namespace is machine-global, so deriving the right *name* proves
    /// nothing about who answered it. Reading the connected handle's owner is
    /// what turns "this is the name my repository derives" into "this is my own
    /// process's endpoint".
    pub fn handle_owner_sid<H: AsRawHandle>(handle: &H) -> Result<String, String> {
        unsafe {
            let mut owner = PSID::default();
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            let status = GetSecurityInfo(
                HANDLE(handle.as_raw_handle()),
                SE_KERNEL_OBJECT,
                OWNER_SECURITY_INFORMATION,
                Some(&mut owner),
                None,
                None,
                None,
                Some(&mut descriptor),
            );
            if status.0 != ERROR_SUCCESS {
                return Err(format!(
                    "failed to read the connected pipe's owner: Win32 error {}",
                    status.0
                ));
            }
            // The owner SID points into the returned descriptor, so it is
            // rendered before that allocation is released.
            let rendered = sid_to_string(owner);
            if !descriptor.is_invalid() {
                let _ = LocalFree(HLOCAL(descriptor.0));
            }
            rendered
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SID_A: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";
    const SID_B: &str = "S-1-5-21-1111111111-2222222222-3333333333-1002";

    #[test]
    fn a_pipe_name_is_the_fixed_prefix_and_a_sha256_digest() {
        let name = pipe_name_for(SID_A, Path::new(r"C:\repo\.git"));
        let digest = name.strip_prefix(PIPE_NAME_PREFIX).expect("prefixed");
        assert_eq!(digest.len(), 64, "{name}");
        assert!(
            digest
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "{name}"
        );
    }

    #[test]
    fn neither_the_user_nor_the_repository_survives_into_the_published_name() {
        // The pipe namespace is machine-global: anything that can enumerate it
        // would otherwise learn a local account's SID and a developer's
        // directory layout from the endpoint alone.
        let name = pipe_name_for(SID_A, Path::new(r"C:\Users\dev\secret-project\.git"));
        assert!(!name.contains("S-1-5-21"), "{name}");
        assert!(!name.to_lowercase().contains("secret-project"), "{name}");
        assert!(!name.to_lowercase().contains("users"), "{name}");
    }

    #[test]
    fn one_repository_and_one_user_derive_one_endpoint() {
        let repeated = pipe_name_for(SID_A, Path::new(r"C:\repo\.git"));
        assert_eq!(repeated, pipe_name_for(SID_A, Path::new(r"C:\repo\.git")));
    }

    /// The lock, the Unix socket, and the pipe share one identity, so every
    /// linked worktree of a repository must reach the same owner.
    #[test]
    fn linked_worktrees_converge_while_repositories_and_users_diverge() {
        let main = pipe_name_for(SID_A, Path::new(r"C:\repo\.git"));

        // A linked worktree's common directory *is* the main repository's, which
        // is what `repo_lock::resolve_common_dir` already guarantees; the point
        // here is that differently spelled canonical forms of it still converge.
        for spelling in [
            r"\\?\C:\repo\.git",
            r"C:/repo/.git",
            r"C:\REPO\.GIT",
            r"\\?\C:/Repo/.git",
        ] {
            assert_eq!(
                pipe_name_for(SID_A, Path::new(spelling)),
                main,
                "{spelling} must derive the repository's one endpoint"
            );
        }

        assert_ne!(
            main,
            pipe_name_for(SID_A, Path::new(r"C:\other\.git")),
            "distinct repositories must not share an endpoint"
        );
        assert_ne!(
            main,
            pipe_name_for(SID_B, Path::new(r"C:\repo\.git")),
            "two local users must not share an endpoint"
        );
    }

    /// Without the NUL the SID and the path would be one concatenated string,
    /// and two different (user, repository) pairs could cut it the same way.
    #[test]
    fn the_separator_keeps_the_two_digest_inputs_from_being_re_cut() {
        let split_one = pipe_name_for("S-1-5-21-1", Path::new(r"\extra\.git"));
        let split_two = pipe_name_for("S-1-5-21-1\\extra", Path::new(r"\.git"));
        assert_ne!(split_one, split_two);
    }

    #[test]
    fn extended_length_and_unc_prefixes_normalize_to_one_form() {
        assert_eq!(
            normalize_common_dir(Path::new(r"\\?\C:\repo\.git")),
            r"c:\repo\.git"
        );
        assert_eq!(
            normalize_common_dir(Path::new(r"\\?\UNC\server\share\repo\.git")),
            r"\\server\share\repo\.git"
        );
        assert_eq!(
            normalize_common_dir(Path::new(r"\\server\share\repo\.git")),
            r"\\server\share\repo\.git"
        );
        assert_eq!(
            normalize_common_dir(&PathBuf::from("C:/Repo/.git")),
            r"c:\repo\.git"
        );
    }

    #[test]
    fn pipe_endpoints_are_scheme_qualified() {
        assert_eq!(
            pipe_endpoint(r"\\.\pipe\cflx-v2-abc"),
            r"pipe://\\.\pipe\cflx-v2-abc"
        );
    }
}
