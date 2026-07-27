# Design: Actionable Acceptance repair contract

## Context

The runtime currently carries `Vec<String>` through parsing, history, follow-up rendering, prompting, and retry comparison. One value serves two incompatible purposes: detailed repair instruction and stable retry identity. Normalization intentionally discards prose for comparison, so storing normalized output back into the detailed slot destroys information Apply needs.

Broad semantic fingerprints answer whether repository content changed, not whether a specific finding was addressed. They therefore cannot authorize another retry for an unchanged defect.

## Decisions

### Structured finding model

Use one shared internal repository-finding type equivalent to:

```json
{
  "id": "acceptance-secret-value-scan",
  "severity": "minor",
  "summary": "Challenge and proof leakage is not tested by value",
  "evidence": [
    "tests/support/relay.ts exposes counts but not issued values"
  ],
  "required_changes": [
    {
      "file": "tests/support/relay.ts",
      "description": "Expose issued challenge and presented proof values to tests"
    }
  ],
  "verification": [
    {
      "file": "runtime/recovery.integration.test.ts",
      "description": "Assert recorded values are absent from serialized audit and operator output"
    }
  ]
}
```

`id` is the authoritative retry identity when valid. It is stable for the same defect across attempts and must not be derived from mutable summary, line numbers, or evidence prose. `severity` is `major` or `minor`; both block PASS. All descriptive arrays are non-empty for structured repository findings. Paths are normalized repository-relative paths and may not escape the workspace.

Legacy string findings remain accepted. Runtime wraps each string with its original complete text and the existing deterministic fallback identity. Compatibility does not permit converting a valid structured finding into a string before Apply.

Malformed structured findings do not silently degrade into path-only tasks. They follow the bounded Acceptance protocol-error path supplied by the dependency change.

### Separate payload, identity, and progress

History and retry context store separate concepts:

- complete latest finding payload for Apply and diagnostics;
- stable finding IDs for reconciliation and retry accounting;
- semantic fingerprint for general diagnostics and legacy behavior.

Updating retry checkpoints or fingerprints cannot mutate the complete payload. The in-memory history is authoritative only for an uninterrupted run. Workspace-local current follow-up preserves the latest actionable repair handoff across interruption; when that evidence is absent or invalid, runtime reruns Acceptance rather than inferring a repair target or PASS.

### Runtime-owned follow-up state

The current follow-up stores immutable finding identity and actionable detail separately from Apply-authored remediation evidence. Runtime exposes three conceptual states:

- `open`: latest Acceptance FAIL reports the ID;
- `remediation_claimed`: Apply records evidence for a repair, pending review;
- `closed`: Acceptance no longer reports the ID or returns PASS.

Apply may only move an item from `open` to `remediation_claimed`. A checkbox or evidence line is not repository-verifiable closure. Ingesting a later FAIL with the same ID reopens it. Runtime removes or closes an absent ID only while ingesting a new canonical Acceptance result.

### Focused Apply repair prompt

The retry prompt contains one untrusted JSON block with complete latest open findings. Priority is explicit:

1. latest open structured findings;
2. runtime repair and evidence instructions;
3. proposal, design, and task files as constraints;
4. other bounded context.

Apply must map each required change and verification item to actual changed files and evidence. It must not explore completed proposal tasks as new work. Additional changes require a stated relationship to an open finding.

### Remediation-to-diff validation

Capture the revision at FAIL and compare it with the revision produced by the repair Apply, including relevant tracked and untracked workspace changes. For every structured finding:

- every `required_changes[].file` must appear in the diff;
- every `verification[].file` must appear in the diff;
- remediation evidence must reference existing paths or commands;
- files outside those sets are retained as diagnostics with Apply's relationship explanation.

This is a coverage gate, not semantic acceptance. Passing it only permits Acceptance to run. Failure creates an evidenced `acceptance_remediation_mismatch` hold and does not invoke Acceptance again.

Legacy findings without declared path sets cannot use strict file coverage; they retain compatibility behavior and are still governed by per-identity repeated-finding stopping.

### Per-finding automatic retry budget

Each open ID gets one automatic repair Apply after its first FAIL observation. At the next Acceptance result:

- absent ID: Acceptance closed it;
- same ID: stop before another Apply with `repeated_acceptance_finding`;
- new ID: grant that new ID one automatic repair opportunity;
- mixed old and new IDs: stop for the repeated ID and retain all IDs in diagnostics; do not partially dispatch Apply.

Unrelated semantic progress never resets an ID's repair budget. Explicit operator retry starts a new operator-authorized attempt through the dependency change's revision-bound retry path and preserves prior diagnostics.

### Diagnostics

Both mismatch and repetition holds retain:

- full finding payload;
- stable ID and occurrence count;
- FAIL and Apply revisions;
- required implementation and verification paths;
- actual changed files and coverage result;
- unrelated changed files and relationship explanations;
- Apply remediation evidence;
- stop reason, resumability, and next action.

These records control only temporary pause/retry routing. They never prove implementation, PASS, archive readiness, or merge eligibility.

## Alternatives Rejected

- Prompt with normalized identities only: compact but removes the required repair.
- Keep broad semantic progress as retry authorization: unrelated tests or comments evade the guard.
- Changed-line limits: penalize legitimate large fixes without proving relevance.
- Let Apply close findings: violates Acceptance ownership and truthful completion.
- Persist all attempt history: unnecessary context growth; latest payload plus bounded diagnostics is sufficient.
- Split schema and retry control into separate changes: creates an unsafe intermediate contract.
