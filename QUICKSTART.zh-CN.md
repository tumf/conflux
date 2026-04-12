# QUICKSTART

[![日本語](https://img.shields.io/badge/日本語-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ja.md) [![English](https://img.shields.io/badge/English-QUICKSTART-blue?style=flat-square)](./QUICKSTART.md) [![简体中文](https://img.shields.io/badge/简体中文-QUICKSTART-blue?style=flat-square)](./QUICKSTART.zh-CN.md) [![Español](https://img.shields.io/badge/Español-QUICKSTART-blue?style=flat-square)](./QUICKSTART.es.md) [![Português%20(BR)](https://img.shields.io/badge/Português%20(BR)-QUICKSTART-blue?style=flat-square)](./QUICKSTART.pt-BR.md) [![한국어](https://img.shields.io/badge/한국어-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ko.md) [![Français](https://img.shields.io/badge/Français-QUICKSTART-blue?style=flat-square)](./QUICKSTART.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-QUICKSTART-blue?style=flat-square)](./QUICKSTART.de.md) [![Русский](https://img.shields.io/badge/Русский-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ru.md) [![Tiếng%20Việt](https://img.shields.io/badge/Tiếng%20Việt-QUICKSTART-blue?style=flat-square)](./QUICKSTART.vi.md)

这是最短指南，帮助你首次安装 `cflx`、配置项目、创建 OpenSpec change，并通过 TUI 完成整个实现流程。

Conflux 的命令名是 `cflx`。

## 0. 前提条件

- 可以使用 Rust / Cargo：[安装 Rust](https://rust-lang.org/tools/install/)
- 可以使用 [Claude Code](https://claude.com/product/claude-code)
- 有一个由 git 管理的项目，例如 `~/myproject`

> Conflux 是一个用于启动和控制 AI 代理的编排器。它本身并不是编码代理。
> 它可以使用 [Claude Code](https://claude.com/product/claude-code)、[OpenCode](https://opencode.ai/)、[Codex](https://developers.openai.com/codex/cli) 等 CLI。
> 本 QUICKSTART 以 Claude Code 为例进行说明。

确认前提条件：

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. 安装 `cflx`

从 crates.io 安装。

```bash
cargo install cflx
```

安装后确认：

```bash
cflx --version
```

## 2. 准备项目

从这里开始，请在项目目录中操作。这里以 `~/myproject` 为例。

Conflux 使用 `git worktree`，因此项目必须由 git 管理。

```bash
cd ~/myproject
```

如果是新项目：

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. 安装 bundled skills

将 Conflux 自带的 bundled skill 作为 Claude Code 用配置安装到项目中。

```bash
cflx install-skills --claude
```

这样会在 `./.claude/skills` 下安装 `cflx-*` skill。

是否提交到 Git，可以稍后与 `.cflx.jsonc` 一起统一决定。

## 4. 创建配置文件

配置文件名是 `.cflx.jsonc`，不是 `.cflx.conf`。

最快的方式是生成模板。

```bash
cflx init
```

这样会在当前目录创建 `.cflx.jsonc`。

## 5. 确认 `.cflx.jsonc`

至少只要其中包含面向你要使用的代理的命令，就可以运行。

Claude Code 模板示例：

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

第一次使用时，直接使用 `cflx init` 生成的内容即可。

## 6. 决定哪些内容放入 Git

在首次设置时，需要决定是否将以下两个内容加入 Git。

- `./.claude/skills/cflx-*`
- `./.cflx.jsonc`

推荐做法：

- 如果希望团队或多台机器之间复现相同行为，就提交这两个文件
- 如果只是本地临时使用，就把这两个都加入 `.gitignore`

如果拿不准，先把这两个都提交也没有问题。只要不要把机密信息直接写进 `.cflx.jsonc`，管理起来就会比较轻松。

如果把两者都加入 `.gitignore`：

```bash
printf ".claude/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

如果把两者都加入仓库：

```bash
git add .claude/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. 创建第一个 change proposal

Conflux 处理的是 OpenSpec 的 change。

即使你还不熟悉 OpenSpec 也没关系。bundled skill 已经安装好了，所以可以让 Claude Code 帮你创建 proposal。

例如：

```text
/cflx-proposal 用 python 显示 hello world
```

这样会生成类似 `openspec/changes/add-hello-world/` 的 change 目录，至少包含以下两个文件。

- `proposal.md`：要变更什么
- `tasks.md`：要实现什么

为了尽快开始，只需快速检查这两个文件，如果没有问题，直接提交即可。

检查要点：

- `proposal.md` 的内容是否是你想做的变更
- `tasks.md` 中的实现任务是否完整且不过多
- 是否混入了无关的变更

如有需要，可修改 proposal 或 tasks；确认内容无误后再提交。

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

详细结构如下：

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

## 8. 确认工作区是否干净

在启动 TUI 之前，先确认工作树是否干净。

```bash
git status
```

如果工作区干净，会像这样：

```text
On branch main
nothing to commit, working tree clean
```

## 9. 启动 TUI

以 TUI 模式启动 Conflux。

```bash
cflx
```

会显示如下界面。

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. 在 TUI 中执行

基本操作：

- `↑/↓` 或 `j/k`：选择 change
- `Space`：标记为执行目标
- `F5`：开始执行
- `Ctrl+C`：退出

最短流程：

1. 启动 `cflx`
2. 移动到想处理的 change
3. 按 `Space` 选择
4. 按 `F5` 执行

这次示例里只有一个 change，所以直接按 `Space` → `F5` 即可执行。

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux 会自动循环执行以下步骤。

- apply
- accept
- archive
- resolve / merge

状态变为 `merged` 后就完成了。

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. 确认结果

确认实现已经写入。

```bash
tree
cat hello.py
```

示例：

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

OpenSpec 侧也已经更新。

```bash
tree openspec -L 10
```

示例：

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

可以看到，change proposal 已被归档，最终规格已提升到 `openspec/specs`。

例如：

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

有了这个 spec，Conflux 就能快速理解软件的行为，并更稳定地推进下一个变更。

---

至此，最简单的实现周期就完成了。

本 QUICKSTART 到此为止，重点是让你以最短路径完成第一次运行。
在实际使用中，你可能还需要 proposal 打磨、配置调整、并行执行、故障排查等更细致的技巧。
后续请参阅 README 或 `cflx --help`。

如有意见或问题，请在 [GitHub Issue](https://github.com/tumf/conflux/issues) 提交，或在 X 上提及 `@tumf`。
