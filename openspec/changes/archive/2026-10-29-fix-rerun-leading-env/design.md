# Design

## Dependency

This change consumes the shared declaration planner and the typed declaration-defect channel created by `fix-required-verification-fail-closed`. Without it, an assignment-only or malformed-assignment command would be silently dropped from the manifest instead of being reported, so the two new defect codes have nowhere truthful to go. The dependency is a repository output consumed by implementation and by the pre-integration gates, not roadmap ordering.

## Representation

```text
VerificationRequest { verification_id, argv: Vec<String> /* executable + args */, env: Vec<(String, String)> /* effective, ordered */, cwd, automation_path }
```

Parsing happens once, inside the planner's request conversion, on the tokenized argv:

1. metacharacter refusal and `shlex::split` exactly as today;
2. consume leading tokens matching `^[A-Za-z_][A-Za-z0-9_]*=`; a leading token that contains `=` but fails the identifier rule is `declaration_invalid_assignment`;
3. no token left → `declaration_missing_command`;
4. fold duplicates: last assignment wins; `env` is the effective map in first-seen order.

`env` the executable needs no special case: it does not match `NAME=` and simply becomes `argv[0]`.

## Child environment

`CommandSupervisor::run` / `run_bounded` take the bindings (`&[(String, String)]`) in addition to argv. `DirectCommandSupervisor` calls `Command::env(name, value)` per binding on the child builder; the owner process never calls `set_var`. In-memory test supervisors record what they were handed so parser-to-supervisor wiring is asserted, and the real-process test uses `printenv NAME` (exit `1` when unset) so correctness is an exit code and an artifact line, not a log grep.

## Identity

- `ManifestGate` and `VerificationEvidence` gain `env_names: Vec<String>` (sorted, distinct) and `env_binding_digest: String` (`#[serde(default)]` on the record so older envelopes read as the empty-bindings digest).
- `ManifestGate::fingerprint_value` includes both fields; `evaluate_reuse` compares `env_binding_digest` as one more binding with mismatch field `env_binding_digest`.
- Canonical digest input: pairs sorted by name, `name NUL value NUL` concatenated, SHA-256, full-length hex. The empty-bindings digest is the digest of the empty byte string.

## Rendering

One renderer (`render_request_for_diagnostics(&VerificationRequest) -> String` and a JSON twin) produces `NAME=<redacted> … executable args…`. Every current `format!` that prints `gate.argv`/`request.argv` for a human or for the reviewer context goes through it. The reviewer context (`build_acceptance_boundary_context`) lists `env_names` only.

## Tool resolution

Unchanged in mechanism; it now receives `argv[0]` after assignments were removed. `tool_unavailable` therefore keeps its meaning: the *executable* is missing.

## Verification strategy

- `--lib` (`rerun_leading_env`): parser table tests on real declarations; supervisor wiring; real `DirectCommandSupervisor` with `printenv`; fingerprint and reuse tests; sentinel-absence tests over every rendered surface listed in the proposal, including the `/api/v2` projection of a gate-failure `error_detail`.
- `tests/run_exit_tests.rs` (`rerun_leading_env`): real `cflx openspec verify <id>` and `--plan --json` on a temp repository with `XDG_STATE_HOME`/`state_base_dir` outside it; assertions on exit status, artifact content, `env_names`, and sentinel absence from command-derived reports and envelope/manifest JSON using a non-echoing child; raw child output is separate.
- Anything over one second is `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`.
