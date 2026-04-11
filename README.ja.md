# Conflux

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

OpenSpec の変更ワークフローを自動化する CLI です。`openspec` と AI コーディングエージェントを連携させ、変更の適用・受け入れ・アーカイブを進めます。

## 主な使い方

| 使い方 | コマンド |
|------|---------|
| TUI | `cflx` |
| ヘッドレス実行 | `cflx run` |

サーバーモード、リモート TUI、REST API、`cflx service` は [サーバーモードガイド](docs/guides/SERVER.ja.md) を参照してください。

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

# bundled skills をインストール
cflx install-skills
```

## 設定

設定ファイルは JSONC です。

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

テンプレート生成:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

詳しい設定例やフック、ワークスペース実行、コマンドキューの説明は英語 README を参照してください。

## インストール

```bash
cargo install cflx
```

## ドキュメント

| ドキュメント | 説明 |
|----------|-------------|
| [QUICKSTART.ja.md](QUICKSTART.ja.md) | 初回セットアップ |
| [サーバーモードガイド](docs/guides/SERVER.ja.md) | サーバーモード、リモート TUI、Web UI、REST API、バックグラウンドサービス |
| [README.md](README.md) | 完全なドキュメント（英語） |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | 使用例 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | コントリビューションガイド |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | 開発ガイド |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | リリースガイド |
| [docs/openapi.yaml](docs/openapi.yaml) | API 仕様 |

## ライセンス

MIT
