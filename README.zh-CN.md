# Conflux

[![日本語](https://img.shields.io/badge/%E8%A8%80%E8%AA%9E-日本語-0f766e?style=flat-square)](./README.ja.md)
[![English](https://img.shields.io/badge/Language-English-2563eb?style=flat-square)](./README.md)
[![简体中文](https://img.shields.io/badge/%E8%AF%AD%E8%A8%80-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-dc2626?style=flat-square)](./README.zh-CN.md)
[![Español](https://img.shields.io/badge/Idioma-Espa%C3%B1ol-f59e0b?style=flat-square)](./README.es.md)
[![Português (BR)](https://img.shields.io/badge/Idioma-Portugu%C3%AAs%20(BR)-16a34a?style=flat-square)](./README.pt-BR.md)
[![한국어](https://img.shields.io/badge/%EC%96%B8%EC%96%B4-%ED%95%9C%EA%B5%AD%EC%96%B4-7c3aed?style=flat-square)](./README.ko.md)
[![Français](https://img.shields.io/badge/Langue-Fran%C3%A7ais-0891b2?style=flat-square)](./README.fr.md)
[![Deutsch](https://img.shields.io/badge/Sprache-Deutsch-4b5563?style=flat-square)](./README.de.md)
[![Русский](https://img.shields.io/badge/%D0%AF%D0%B7%D1%8B%D0%BA-%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-b91c1c?style=flat-square)](./README.ru.md)
[![Tiếng Việt](https://img.shields.io/badge/Ng%C3%B4n%20ng%E1%BB%AF-Ti%E1%BA%BFng%20Vi%E1%BB%87t-ea580c?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux 是一个基于规格驱动开发、用于编排 AI 编码代理自主开发流程的工具。即使没有人工持续盯着，它也会持续推进变更，按一整套流程完成应用、验收判定、归档，直到最终合并。

它追求的并不是一次性的代码生成，而是先定义规格，再沿着该规格不断累积变更，持续培育一个面向实际运行、具备一定规模的成品。

此外，Conflux 不依赖特定的 AI 供应商。它被设计为可在 [Claude Code](https://docs.anthropic.com/en/docs/claude-code)、[Codex](https://openai.com/index/openai-codex/) 和 [OpenCode](https://opencode.ai/) 等工具之间灵活切换使用。

## Conflux 的核心概念

- **睡觉时也能推进的自主开发**：即使没有人工持续值守，AI 代理也会按顺序处理变更，推动开发不断前进。
- **规格驱动开发**：使用 [OpenSpec](https://github.com/openspec/openspec)，先定义规格，再依据规格推进实现、验收与改进。
- **持续培育具备一定规模的成品**：不会停留在一次性生成，而是通过不断累积变更，逐步逼近完整成品。

## 实现这一目标的机制

- **多重 Ralph 循环**：在反复迭代中持续改进，并将每轮迭代之间传递的上下文压缩到最小，以更高效地使用 LLM。
- **使用 git worktree 的并行开发**：Conflux 为每个 change 分配独立的 worktree，从而让多个变更能够安全并行推进。
- **可自由选择、与厂商无关的代理**：不被固定绑定到 [Claude Code](https://docs.anthropic.com/en/docs/claude-code)、[Codex](https://openai.com/index/openai-codex/) 或 [OpenCode](https://opencode.ai/) 等特定供应商，可根据目的替换实现代理或评估代理。
- **实现与验收职责分离**：将推动实现的角色与验收成果的角色分开后，就可以将高速度的编码者与更聪明的评审者结合起来。这不仅能更高效地利用 LLM，也能提升整体开发速度。

简而言之，Conflux 是一个**用于将基于规格驱动开发的自主开发，以具备并行执行和职责分离能力的现实工作流加以运行，并持续推动具备一定规模的成品前进的编排器**。

## 主要用法

| 用法 | 命令 |
|------|---------|
| TUI | `cflx` |
| 无头执行 | `cflx run` |

常用 TUI 按键：

| 按键 | 操作 |
|-----|------|
| `Space` | 标记或取消标记 change |
| `F5` | 开始、恢复、重试或继续处理 |
| `x` | 处理运行中时，将符合条件的 `not queued` change 加入队列 |

关于服务器模式、远程 TUI、REST API 和 `cflx service`，请参阅[服务器模式指南（英文）](docs/guides/SERVER.md)。

## 快速开始

首次设置请参阅 [QUICKSTART.zh-CN.md](QUICKSTART.zh-CN.md)。

## 基本命令

```bash
# TUI
cflx

# 无头执行
cflx run

# 仅执行特定变更
cflx run --change add-feature-x

# 初始化配置文件
cflx init

# 安装 bundled skills
cflx install-skills

# 为 Claude Code 安装 bundled skills
cflx install-skills --claude

# 为 Claude Code 全局安装 bundled skills
cflx install-skills --claude --global
```

## 配置

配置文件采用 JSONC 格式。

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

TUI 用户偏好会有意与编排配置分开。默认的开始、恢复、重试、继续键是 `F5`；如需仅覆盖本地 TUI 的 start 绑定，请在 `~/.config/cflx/tui.jsonc` 中设置：

```jsonc
{
  "keybindings": {
    "start": ["F5", "!"]
  }
}
```

生成配置模板：

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

关于更详细的配置示例、hooks、工作区执行以及命令队列说明，请参阅 [docs/guides/USAGE.md](docs/guides/USAGE.md)。

## 安装

```bash
cargo install cflx
```

## 文档

| 文档 | 说明 |
|----------|-------------|
| [QUICKSTART.zh-CN.md](QUICKSTART.zh-CN.md) | 首次设置 |
| [服务器模式指南（英文）](docs/guides/SERVER.md) | 服务器模式、远程 TUI、Web UI、REST API、后台服务 |
| [README.md](README.md) | 完整文档（英文） |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | 使用示例 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 贡献指南 |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | 开发指南 |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | 发布指南 |
| [docs/openapi.yaml](docs/openapi.yaml) | API 规范 |

## 许可证

MIT
