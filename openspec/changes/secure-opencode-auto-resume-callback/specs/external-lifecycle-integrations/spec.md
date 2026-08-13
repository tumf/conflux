## ADDED Requirements

### Requirement: Reference OpenCode completion callback is loopback-confined and recoverably deduplicated

The reference OpenCode completion callback MUST validate its configured base URL as loopback HTTP, MUST resolve the callback path against that base, and MUST verify that the resolved URL retains the base's origin before sending. Any path that changes origin, including absolute, protocol-relative, or backslash variants, MUST be rejected. The callback MUST NOT follow redirects.

It MUST use an atomic local in-flight claim so concurrent invocations for the same execution event produce at most one POST during normal operation. A successful-delivery marker MUST be distinct from the in-flight claim. Failed POST MUST release the claim so a later external invocation may retry. A fresh in-flight claim MUST return a distinct non-success outcome. An existing successful-delivery marker MUST return success without posting. A claim older than five minutes MAY be atomically taken over so a crashed process cannot suppress delivery permanently.

Normal operation is at-most-once. If a process crashes after a successful POST but before atomic promotion to the success marker, stale takeover MAY redeliver and crash recovery is at-least-once. Exactly-once delivery is not promised.

These adapter records are observability and delivery state only. They MUST NOT alter Conflux workflow routing or change repository-verifiable completion.

#### Scenario: Absolute path cannot replace loopback base

- **GIVEN** the callback is configured with a loopback base URL
- **WHEN** its path argument is absolute, protocol-relative, a backslash origin variant, or resolves to a different origin
- **THEN** the callback rejects before sending HTTP
- **AND** no successful-delivery marker is written

#### Scenario: Redirect cannot leave loopback

- **GIVEN** the callback sends to a loopback endpoint
- **WHEN** the endpoint returns an HTTP redirect
- **THEN** the callback treats it as failure without following the redirect
- **AND** no request is sent to the redirect destination

#### Scenario: Concurrent callbacks deliver at most once

- **GIVEN** two callback processes receive the same execution event concurrently
- **WHEN** both attempt to claim delivery
- **THEN** atomic claim creation permits at most one process to POST
- **AND** the other process reports a distinct non-success in-flight outcome

#### Scenario: Failed delivery remains retryable

- **GIVEN** a callback owns the in-flight claim but its POST fails
- **WHEN** the process settles
- **THEN** it does not write a successful-delivery marker
- **AND** it releases the in-flight claim
- **AND** a later invocation may claim and attempt delivery

#### Scenario: Successful delivery remains deduplicated

- **GIVEN** the OpenCode POST succeeds
- **WHEN** a later callback receives the same execution event
- **THEN** it observes the successful-delivery marker and does not POST again
- **AND** it exits successfully

#### Scenario: Stale in-flight claim does not permanently suppress delivery

- **GIVEN** an in-flight claim whose owning process died without settling
- **WHEN** a later invocation finds the claim older than five minutes
- **THEN** it atomically takes over the claim and attempts delivery
- **AND** a fresh claim remains refused with a non-success in-flight outcome

#### Scenario: Crash after POST can redeliver observably

- **GIVEN** a process crashes after POST succeeds but before success-marker promotion
- **WHEN** its claim becomes stale and a later invocation takes it over
- **THEN** the later invocation may POST again
- **AND** automation-marker evidence makes the duplicate resume observable
- **AND** Conflux workflow completion remains unchanged
