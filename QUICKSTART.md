# QUICKSTART

This guide shows the shortest path to install `cflx`, set up a project, generate an OpenSpec change, and complete an implementation through the TUI.

Conflux is implemented as the `cflx` command.

## 0. Prerequisites

- Rust / Cargo is installed: [Install Rust](https://rust-lang.org/tools/install/)
- [Claude Code](https://claude.com/product/claude-code) is installed
- You have a git-managed project, such as `~/myproject`

> Conflux orchestrates AI agents. It is not itself an AI coding agent.
> It can drive CLIs such as [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/), and [Codex](https://developers.openai.com/codex/cli).
> This QUICKSTART uses Claude Code as the example.

Verify your environment:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. Install `cflx`

Install from crates.io:

```bash
cargo install cflx
```

Verify the installation:

```bash
cflx --version
```

## 2. Prepare a project

From this point on, work inside your project directory. This guide uses `~/myproject` as an example.

Conflux uses `git worktree`, so your project must be managed by git.

```bash
cd ~/myproject
```

If you are starting from scratch:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. Install bundled Conflux skills

Install the bundled `cflx-*` skills into the project:

```bash
cflx install-skills
```

This installs skills under `./.agents/skills`.

Whether to ignore or commit them is best decided together with `.cflx.jsonc` a little later.

## 4. Create a config file

The config file is named `.cflx.jsonc`, not `.cflx.conf`.

Generate it from the default template:

```bash
cflx init
```

This creates `.cflx.jsonc` in the current directory.

## 5. Check `.cflx.jsonc`

At minimum, the config must contain working commands for your chosen agent.

Example Claude Code template:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

For a first run, the generated file from `cflx init` is usually enough.

## 6. Decide what to track in Git

During initial setup, decide whether these two things should be tracked in Git:

- `./.agents/skills/cflx-*`
- `./.cflx.jsonc`

Recommended defaults:

- If you want reproducible behavior across teammates or machines, commit both
- If this is purely local and disposable, ignore both with `.gitignore`

If you are unsure, committing both is a reasonable default. It works well as long as you do not put secrets directly into `.cflx.jsonc`.

Ignore both with `.gitignore`:

```bash
printf ".agents/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

Or commit both to the repository:

```bash
git add .agents/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. Create your first change proposal

Conflux works on OpenSpec changes.

If you are not familiar with OpenSpec, that is fine. Since the bundled skills are already installed, you can ask Claude Code to generate the proposal for you.

For example:

```text
/cflx-proposal create a Python program that prints hello world
```

This generates a change directory such as `openspec/changes/add-hello-world/`, including at least:

- `proposal.md`: what should change
- `tasks.md`: what should be implemented

For the shortest path, just skim these two files and commit them if they look right.

What to check:

- `proposal.md` matches the change you actually want
- `tasks.md` lists the implementation work at the right level
- no unrelated changes are mixed in

Review and edit these files if needed, then commit the proposal:

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

The structure will look like this:

```text
openspec
└── changes
    └── add-hello-world
        ├── proposal.md
        ├── specs
        │   └── hello-world
        │       └── spec.md
        └── tasks.md
```

## 8. Make sure the workspace is clean

Before launching the TUI, check that the working tree is clean:

```bash
git status
```

A clean workspace should look like this:

```text
On branch main
nothing to commit, working tree clean
```

## 9. Launch the TUI

Start Conflux in TUI mode:

```bash
cflx
```

You should see a screen like this:

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. Execute from the TUI

Basic keys:

- `↑/↓` or `j/k`: move between changes
- `Space`: mark a change for execution
- `F5`: start processing
- `Ctrl+C`: quit

Shortest path:

1. Launch `cflx`
2. Move to the change you want to run
3. Press `Space`
4. Press `F5`

In this example there is only one change, so press `Space`, then `F5`.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux will drive the full loop:

- apply
- accept
- archive
- resolve / merge

When the change reaches `merged`, it is done.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. Check the result

Your project should now contain the implementation:

```bash
tree
cat hello.py
```

Example:

```text
.
├── hello.py
└── openspec
    ├── changes
    └── specs
```

```python
print("hello world")
```

The OpenSpec files will also be updated.

```bash
tree openspec -L 10
```

Example:

```text
openspec
├── changes
│   └── archive
│       └── add-hello-world
│           ├── proposal.md
│           ├── specs
│           │   └── hello-world
│           │       └── spec.md
│           └── tasks.md
└── specs
    └── hello-world
        └── spec.md
```

The change proposal has been archived, and the resulting specification has been promoted into `openspec/specs`.

For example:

```bash
cat openspec/specs/hello-world/spec.md
```

```markdown
## Requirements

### Requirement: hello-world-output

The program must print "hello world" to standard output when executed.

#### Scenario: default-execution

**Given**: The user has Python installed
**When**: The user runs `python hello.py`
**Then**: The program prints `hello world` to stdout and exits with code 0
```

With this spec in place, Conflux can quickly understand the software behavior and continue development from a stable specification.

---

This completes the simplest end-to-end implementation cycle.

This QUICKSTART intentionally stops at getting your first run working as quickly as possible.
In real usage, you may need more detailed techniques around proposal refinement, config tuning, parallel execution, and troubleshooting.
For what comes next, see the README and `cflx --help`.

For feedback or questions, please open a [GitHub Issue](https://github.com/tumf/conflux/issues) or mention `@tumf` on X.
