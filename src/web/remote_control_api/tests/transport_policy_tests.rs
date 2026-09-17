//! Which transports the argv-bearing resources trust, and which they never do.
//!
//! The two sensitive surfaces — proposal subscriptions and execution sinks —
//! store and disclose a command this owner will execute. They ask exactly one
//! question about the channel a request arrived on, and they ask it through one
//! predicate, so a platform can never be trusted by one of them and refused by
//! the other. These tests pin that predicate rather than either handler: the
//! handlers are proven against a real router elsewhere, and what must not drift
//! is the *classification*.

use axum::Extension;

use crate::web::remote_control_api::{is_owner_only_local, ApiTransport};

#[test]
fn both_owner_only_local_transports_are_trusted_and_tcp_is_not() {
    // The Unix socket earns it from mode 0600 under the Git common directory;
    // the Named Pipe from a protected DACL granting the current user alone and
    // kernel-level rejection of remote clients.
    assert!(ApiTransport::Unix.is_owner_only_local());
    assert!(ApiTransport::NamedPipe.is_owner_only_local());

    // Loopback included: a bearer token proves a caller read a secret, not that
    // it is this machine's owner.
    assert!(!ApiTransport::Tcp.is_owner_only_local());
}

#[test]
fn a_request_with_no_transport_marker_is_never_trusted() {
    // A router assembled without the per-listener extension has no proof of
    // transport, and no evidence must not be read as evidence of the safer
    // channel.
    assert!(!is_owner_only_local(&None));

    for transport in [ApiTransport::Unix, ApiTransport::NamedPipe] {
        assert!(
            is_owner_only_local(&Some(Extension(transport))),
            "{transport:?}"
        );
    }
    assert!(!is_owner_only_local(&Some(Extension(ApiTransport::Tcp))));
}
