---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/install_skills.rs
  - tests/install_skills_test.rs
  - openspec/specs/cli/spec.md
  - README.ja.md
---

# Change: Add Claude install-skills targets

**Change Type**: implementation

## Problem/Context

- 現在の `cflx install-skills` は project scope で `./.agents/skills`、`--global` で `~/.agents/skills` に bundled skills を配置する。
- このリポジトリでは `cflx-*` skills はバイナリへ embed され、`install-skills` はそれらを標準スキル配置先へ展開する運用になっている。
- ユーザーは Claude Code 向けの配置先として、`cflx install-skills --claude` を `./.claude/skills`、`cflx install-skills --claude --global` を `~/.claude/skills` に切り替える挙動を求めている。
- 既存の `.agents` 向け挙動は継続利用されているため、Claude 向け追加は後方互換な opt-in フラグとして提供する必要がある。

## Proposed Solution

- `install-skills` に `--claude` フラグを追加し、install root を `.agents` ではなく `.claude` に切り替えられるようにする。
- `--claude` 未指定時の挙動は変更せず、既存の project/global install 先と lock file を維持する。
- `--claude` 指定時は skills install path と lock file path を `.claude` 配下へ揃えて切り替える。
- CLI help、README、関連テストを更新し、4 通りの install target (`project/global` × `agents/claude`) を明示する。

## Acceptance Criteria

- `cflx install-skills --claude` は bundled skills を `./.claude/skills` に展開し、lock file を `./.claude/.skill-lock.json` に書き込む。
- `cflx install-skills --claude --global` は bundled skills を `~/.claude/skills` に展開し、lock file を `~/.claude/.skill-lock.json` に書き込む。
- `cflx install-skills` と `cflx install-skills --global` は従来どおり `.agents` 配下を使い続ける。
- CLI help と user-facing documentation は `--claude` の意味と install 先を明示する。
- install path 解決と lock file 更新の回帰テストが `.agents` / `.claude` の両方をカバーする。

## Out of Scope

- Claude 向け以外の新しい install target 追加
- bundled skill の内容や embed 方法の変更
- `install-skills` の source model 再導入
