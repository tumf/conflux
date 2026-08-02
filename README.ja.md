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

Conflux は、仕様駆動開発に基づく AI コーディングエージェントの自走開発をオーケストレーションするツールです。人が張り付かなくても変更を回し続け、適用、受け入れ判定、アーカイブ、最終的なマージまでを一連の流れとして進めます。

目指しているのは、単発のコード生成ではありません。仕様を先に定め、その仕様に沿って変更を積み上げながら、実運用を見据えた一定規模の完成品を継続的に育てていくことです。

また、Conflux は特定の AI ベンダーに依存しません。[Claude Code](https://docs.anthropic.com/ja/docs/claude-code)、[Codex](https://openai.com/index/openai-codex/)、[OpenCode](https://opencode.ai/) などを入れ替えながら使えるように設計されています。

## Conflux の基本コンセプト

- **寝ている間に進む自走開発**: 人が張り付かなくても、AI エージェントが変更を順に処理し、開発を前に進めます。
- **仕様駆動開発**: [OpenSpec](https://github.com/openspec/openspec) を使って、まず仕様を定め、その仕様に沿って実装、検収、改善を進めます。
- **一定規模の完成品を継続的に育てる**: 単発の生成で終わらず、変更を積み上げながら完成品に近づけていきます。

## それを実現するための仕組み

- **多重 Ralph ループ**: 反復を重ねながら改善し、各イテレーションで引き継ぐコンテキストを最小限に抑えて、LLM を効率よく使います。
- **git worktree を使った並列開発**: Conflux が change ごとに独立した worktree を割り当てることで、複数の変更を安全に並列で進められます。
- **ベンダー非依存でエージェントを選べる**: [Claude Code](https://docs.anthropic.com/ja/docs/claude-code)、[Codex](https://openai.com/index/openai-codex/)、[OpenCode](https://opencode.ai/) など特定ベンダーに固定されず、目的に応じて実装役や評価役を差し替えられます。
- **実装と検収の役割分離**: 実装を前に進める役と、成果物を検収する役を分けることで、速いコーダーと賢いレビュー役を組み合わせられます。これにより、LLM をより効率的に使いながら、開発全体のスピードも高められます。

要するに Conflux は、**仕様駆動開発に基づく自走開発を、並列実行と役割分離を備えた現実的な開発フローとして運用し、一定規模の完成品を継続的に前進させるためのオーケストレータ**です。

## 主な使い方

| 使い方 | コマンド |
|------|---------|
| TUI | `cflx` |
| ヘッドレス実行 | `cflx run` |

便利な TUI キー:

| キー | 操作 |
|-----|------|
| `Space` | change をマーク、またはマーク解除 |
| `F5` | 処理の開始、再開、リトライ、継続 |
| `x` | 処理中に対象の `not queued` change をキューへ追加 |

Web 監視 UI、REST API、`/api/v2` は [Web UI ガイド](docs/guides/WEBUI.ja.md) を参照してください。

## クイックスタート

初回セットアップは [QUICKSTART.ja.md](QUICKSTART.ja.md) を参照してください。

## 基本コマンド

```bash
# TUI
cflx

# ヘッドレス実行
cflx run

# 特定の変更だけ実行
cflx run --change add-feature-x

# 設定ファイルを初期化
cflx init

# bundled skills を .agents/skills にインストール
cflx install-skills

# bundled skills を .claude/skills にインストール
cflx install-skills --claude

# bundled skills を ~/.claude/skills にインストール
cflx install-skills --claude --global
```

## 設定

設定ファイルは JSONC です。

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

TUI のユーザー設定は orchestration 設定とは意図的に分離されています。開始、再開、リトライ、継続の既定キーは `F5` です。ローカル TUI の start binding だけを変える場合は `~/.config/cflx/tui.jsonc` で上書きします:

```jsonc
{
  "keybindings": {
    "start": ["F5", "!"]
  }
}
```

テンプレート生成:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

詳しい設定例やフック、ワークスペース実行、コマンドキューの説明は [docs/guides/USAGE.md](docs/guides/USAGE.md) を参照してください。

## インストール

```bash
cargo install cflx
```

## ドキュメント

| ドキュメント | 説明 |
|----------|-------------|
| [QUICKSTART.ja.md](QUICKSTART.ja.md) | 初回セットアップ |
| [Web UI ガイド](docs/guides/WEBUI.ja.md) | Web UI、REST API、`/api/v2`、サーバーモードからの移行 |
| [README.md](README.md) | 完全なドキュメント（英語） |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | 使用例 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | コントリビューションガイド |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | 開発ガイド |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | リリースガイド |
| [docs/openapi.yaml](docs/openapi.yaml) | API 仕様 |

## ライセンス

MIT
