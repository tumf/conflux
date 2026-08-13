# Design

## Network confinement

The final URL, not only the base, is the security boundary. The base is loopback HTTP. The resolved URL must retain the validated base's origin, which also rejects absolute, protocol-relative, and backslash-variant origin changes. Redirect following is disabled so a loopback server cannot bounce a callback to another address. Any redirect is a bounded delivery failure and tests prove the redirect destination receives no connection.

## Dedupe state machine

Use atomic exclusive creation for an in-flight claim. Only one process can own it. Successful POST atomically promotes delivery to a success marker. Failed POST removes the in-flight claim, allowing a later external invocation to retry. A fresh in-flight claim returns a distinct non-success outcome, while an existing success marker returns success without posting.

A claim older than five minutes is stale and may be atomically replaced by a later invocation. Claim takeover tests use controlled file timestamps rather than wall-clock races. Existing success markers remain the durable local dedupe authority for this example adapter.

## Crash windows

The claim/marker pair cannot provide exactly-once delivery. During normal operation it is at-most-once. If a process crashes after POST succeeds but before atomic promotion to the success marker, stale takeover may redeliver; crash recovery is therefore at-least-once. The automation marker keeps a duplicate resume observable and harmless.

The callback does not loop or retry internally. Core completion remains unaffected by callback failure.
