# code-maintenance Specification

## Purpose
TBD - created by archiving change refactor-codebase-cleanup. Update Purpose after archive.
## Requirements
### Requirement: コマンド実行ロジックの共通化
オーケストレーターは `jj`/シェル実行に関する重複ロジックを共通ヘルパーへ集約し、既存の出力・エラー扱いを維持するために SHALL 共通ヘルパーを使用しなければならない。

#### Scenario: jj 実行の失敗時に既存同等のエラーを返す
- **WHEN** 共通ヘルパーで `jj` コマンドが非0終了する
- **THEN** 既存と同等のエラーメッセージが返される

### Requirement: レガシー／未使用コードの整理
オーケストレーターは未使用のレガシーモジュールや `#[allow(dead_code)]` で保護された不要コードを削除または明示的に隔離するために MUST 整理方針を適用しなければならない。

#### Scenario: 未使用コードを整理した後でもビルドが成功する
- **WHEN** 未使用コードの整理後にビルドを実行する
- **THEN** `cargo build` が成功する

### Requirement: リファクタリング安全性の担保
オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。

#### Scenario: 既存の検証が通過する
- **WHEN** `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` を実行する
- **THEN** すべて成功する

### Requirement: VCS Abstraction Layer

システムは VCS バックエンド（Git, Jujutsu）を統一されたトレイトベースの抽象化で管理しなければならない (SHALL)。

各 VCS 実装は専用サブモジュール (`src/vcs/jj/`, `src/vcs/git/`) に配置しなければならない (MUST)。
共通ロジックは `src/vcs/commands.rs` に集約すること。

#### Scenario: 新しい VCS バックエンドを追加する場合

- **WHEN** 開発者が新しい VCS バックエンドを追加する
- **THEN** `src/vcs/<backend>/` にモジュールを作成し、`WorkspaceManager` トレイトを実装するだけで統合可能

#### Scenario: VCS コマンド実行エラー

- **WHEN** VCS コマンドが失敗する
- **THEN** システムは `VcsError` 型で統一されたエラーを返す
- **AND** エラーにはバックエンド種別と詳細メッセージが含まれる

