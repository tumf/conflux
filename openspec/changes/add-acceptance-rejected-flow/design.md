## Context

Acceptance の `Blocked` 判定は「仕様レベルの差し戻し」を意味する。現状はこれが error として扱われ、ワークスペースが残り、ユーザーの手動介入を待つ状態になる。実際にはリトライで解決する性質のものではないため、正常系（Pass → Archive → Resolve → Merged）と同構造の「Rejected 終端フロー」を導入する。

## Goals / Non-Goals

- Goals:
  - Blocked をエラーではなく明確な終端状態として扱う
  - 差し戻し理由を REJECTED.md として base にコミットする
  - resolve まで完了し、worktree を削除する
  - 再 queue を二重に防止する（ファイルシステム + reducer）

- Non-Goals:
  - rejected change の自動修正や再提出ワークフロー
  - Blocked パース層（`acceptance.rs`）の変更

## Decisions

### Rejection フローの配置

`src/orchestration/rejection.rs` に新規モジュールとして配置する。archive.rs と同レベルの操作（ファイル生成、git 操作、resolve 呼び出し）であり、同じ抽象度で並べるのが自然。

### REJECTED.md の配置

`openspec/changes/<change_id>/REJECTED.md` に配置する。change ディレクトリ内に置くことで、`list_changes_native()` の既存スキャンロジック内でシンプルに検出できる。

### 再 queue 防止の二重ガード

1. **ファイルシステムレイヤー**: `list_changes_native()` が REJECTED.md の存在で change をスキップ → そもそも候補に上がらない
2. **Reducer レイヤー**: `TerminalState::Rejected` を permanent terminal として `AddToQueue` を NoOp に → TUI 手動追加も防止

### TerminalState::Rejected(String)

reason を String で保持する。TUI の error_message() と同様のパターンで、rejected_reason() メソッドを提供する。

### Resolve の呼び方

正常系と同じ `openspec resolve <change_id>` をそのまま使用する。rejected 固有のオプションは不要。

## Risks / Trade-offs

- **Risk**: rejection フロー中の git 操作失敗（base チェックアウト、コミット等）
  - Mitigation: archive フローと同様のエラーハンドリングパターンを適用。失敗時は error 状態にフォールバック。

- **Risk**: resolve コマンドが rejected change に対して想定外の動作をする可能性
  - Mitigation: resolve は change ディレクトリの存在のみを前提にしているため、REJECTED.md の有無は影響しない。

## Migration Plan

既存の `Blocked` ケースはすべて自動的に新しい Rejected フローに移行する。データマイグレーションは不要（runtime state のみ）。
