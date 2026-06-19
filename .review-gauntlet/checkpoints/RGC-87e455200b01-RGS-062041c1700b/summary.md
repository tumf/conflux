# Review Gauntlet Checkpoint

- Checkpoint state: complete
- Usable as review base: True
- Review base commit: 87e455200b01e37d863ea42bf8ecedab9b1f1a12
- Session ID: RGS-062041c1700b
- Created at: 2026-06-19T06:28:42.641312Z (generation timestamp)

## Coverage

| State | Count |
| --- | ---: |
| reviewed | 620 |

## Findings

| ID | State | Path | Rule | Content |
| --- | --- | --- | --- | --- |
| RGF-0001 | confirmed | .cflx.jsonc | cli-contract | The on_merged hook config disables git verification hooks for the version-bump commit. This bypasses repository commit hooks during an automated merge lifecycle step, which can let invalid generated commits land unnoticed unless there is a documented, compensating check elsewhere. |
| RGF-0002 | confirmed | .github/workflows/ci.yml | ci-reproducibility | The CI workflow depends on floating action refs and mutable runner/toolchain selectors, which makes successful builds non-reproducible and exposes the workflow to unexpected upstream changes. Pin third-party actions and toolchains to immutable versions or SHAs, and use explicit runner images where practical. |
| RGF-0003 | confirmed | .github/workflows/ci.yml | ci-reproducibility | cargo-audit is installed without a version constraint, so every CI run may test with a different audit binary. Pin the cargo-audit version or install from a checked-in tool manifest to keep audit results reproducible. |
| RGF-0004 | confirmed | .github/workflows/release.yml | ci-reproducibility | The release matrix uses `macos-latest` for the x86_64 Darwin artifact, so the runner image can change underneath release builds and alter toolchains, SDKs, or packaging behavior. Pin the macOS image to a concrete version to keep release artifacts reproducible. |
| RGF-0005 | confirmed | .github/workflows/release.yml | ci-reproducibility | Rust is installed from the moving `stable` channel, which means a tag rebuild at a later date may use a different compiler and produce different artifacts or failures. Pin the Rust toolchain version or read it from a committed rust-toolchain file. |
| RGF-0006 | confirmed | .github/workflows/release.yml | ci-reproducibility | Node.js is configured as the floating major version `20`, so dashboard builds can change when GitHub resolves a newer Node 20 patch/minor. Pin the exact Node version used for release builds. |
| RGF-0007 | confirmed | .gitignore | data-validation | `.review-gauntlet/` ignores the directory itself, so the later negation cannot re-include `.review-gauntlet/checkpoints/`. Git does not re-include files under an ignored parent directory; use a contents-only ignore pattern and explicitly unignore the checkpoint subtree. |
| RGF-0008 | confirmed | .node-version | docs-accuracy | This pins local development to Node 24.12.0, but both CI and release workflows build the dashboard with Node 20. Contributors using this file can install dependencies or generate lockfile output under a different Node major than the one used in automation, making local verification diverge from CI. |
| RGF-0009 | dismissed | .opencode/agent/code.md | docs-accuracy | OpenCode loads project markdown agents from `.opencode/agents/` (plural), but this file is under `.opencode/agent/`, so the agent definition will not be discovered as documented. |
| RGF-0010 | confirmed | .opencode/agent/code.md | docs-accuracy | The frontmatter uses the deprecated `tools` option. OpenCode documentation recommends using `permission` for new or updated agent configs, so this should be updated to avoid stale configuration guidance. |
| RGF-0011 | confirmed | .opencode/agent/code.md | docs-accuracy | This workflow tells the agent to plan with TodoWrite, but the repository AGENTS.md explicitly says to use `bd` for all task tracking and not TodoWrite. That makes the agent instructions inconsistent with the repo's required workflow. |
| RGF-0012 | confirmed | .opencode/agent/spec.md | docs-accuracy | This project-specific markdown agent file is under `.opencode/agent/`, but OpenCode documents project agents as being loaded from `.opencode/agents/`. As placed, this spec agent is likely not discovered by OpenCode, so the documented `/cflx-proposal` handoff workflow will not be available as intended. |
| RGF-0013 | dismissed | .opencode/commands/cflx-apply.md | docs-accuracy | The Goal path contains a typo (`openspec/chagens/{change_id}/tasks.md`), which can mislead apply agents away from the actual OpenSpec change directory used elsewhere in the prompt (`changes/<id>/...`). |
| RGF-0014 | confirmed | .opencode/commands/cflx-archive.md | docs-accuracy | This command prompt still directs agents to use the upstream `npx @fission-ai/openspec@latest` CLI for listing, showing, archiving, and validating. In this repository the archive workflow is documented and specified around the native `cflx openspec` commands, so following this prompt can bypass Conflux's repository-specific archive behavior and diverge from the embedded `cflx-archive` skill reference. |

## Triage Events

| Event | Finding | From | To | Reason |
| ---: | --- | --- | --- | --- |
| 1 | RGF-0001 | open | confirmed | Confirmed: .cflx.jsonc line 12 sets git_commit_no_verify=true for the on_merged make bump-patch hook, so the generated version-bump commit bypasses repository git verification hooks. |
| 2 | RGF-0002 | open | confirmed | ci.yml uses mutable selectors: ubuntu-latest/macos-latest runner labels, dtolnay/rust-toolchain@stable, and unpinned third-party action major tags; these can change CI behavior across runs. |
| 3 | RGF-0003 | open | confirmed | ci.yml installs cargo-audit with cargo install cargo-audit --locked but without --version, so the binary version can change between CI runs. |
| 4 | RGF-0004 | open | confirmed | Real reproducibility issue: release matrix uses floating macos-latest for x86_64-apple-darwin at .github/workflows/release.yml:25, so a later tag rebuild may run on a different macOS image. |
| 5 | RGF-0005 | open | confirmed | Real reproducibility issue: .github/workflows/release.yml:87-90 installs Rust from dtolnay/rust-toolchain@stable, so later rebuilds may use a different Rust compiler. |
| 6 | RGF-0006 | open | confirmed | Real reproducibility issue: .github/workflows/release.yml:92-95 configures Node.js as floating major '20', so later dashboard builds may use a different Node.js patch/minor. |
| 7 | RGF-0007 | open | confirmed | Real issue: ignoring .review-gauntlet/ prevents Git from recursing into the directory, so the later negation cannot re-include .review-gauntlet/checkpoints/. |
| 8 | RGF-0008 | open | confirmed | Real mismatch: .node-version pins local development to 24.12.0 while CI and release workflows configure actions/setup-node with node-version '20'. |
| 9 | RGF-0009 | open | dismissed | Non-issue: current OpenCode agent guidance in this environment recognizes both .opencode/agent/<name>.md and .opencode/agents/<name>.md as valid project agent locations; therefore this file being under singular .opencode/agent/ is not enough to make it unloaded. |
| 10 | RGF-0010 | open | confirmed | OpenCode agents documentation marks the tools option as deprecated and recommends permission for new or updated configs; this file still uses tools at lines 5-6. |
| 11 | RGF-0011 | open | confirmed | AGENTS.md requires bd for all task tracking and explicitly forbids TodoWrite/TaskCreate/markdown TODO lists, while this agent prompt tells implementers to plan with TodoWrite at lines 90-91. |
| 12 | RGF-0012 | open | confirmed | OpenCode docs specify per-project markdown agents must be placed under .opencode/agents/, while the target file is under the singular .opencode/agent/ path. |
| 13 | RGF-0013 | open | dismissed | Line 15 currently reads 'openspec/changes/{change_id}/tasks.md' — the typo 'chagens' no longer exists in the file. The content digest may have changed since the finding was created. No action needed. |
| 14 | RGF-0014 | open | confirmed | The archive command still instructs agents to invoke upstream npx @fission-ai/openspec@latest for list/show/archive/validate, while this repository's Conflux workflow should use the project-local OpenSpec conventions/CLI wrapper rather than the upstream package; lines 21-31 therefore document the wrong operational path. |

## Blockers

None
