# Change: Parallel実行中の衝突解決でTUIに「resolving」ステータスを表示する

## なぜ（Why）
Parallel実行でmerge衝突が発生し、自動的に衝突解決（`resolve_conflicts_with_retry` / `resolve_merges_with_retry`）が実行される際、TUIには「resolving」ステータスが表示されない。

現状の問題：
- `src/parallel/conflict.rs` は `ConflictResolutionStarted` イベントを送信するが、このイベントには **change_id が含まれていない**
- TUI側（`src/tui/state/events.rs`）で `QueueStatus::Resolving` に遷移するのは `ResolveStarted { change_id }` イベントのみ
- `ConflictResolutionStarted` は TUI で無視されるため（`_ => {}` に落ちる）、ログには出るが状態表示は変わらず、ユーザーは「どの change が解決中か」が分からない

動作は正常だが、表示が不完全なため、ユーザー体験が損なわれている。

## 何を変えるか（What Changes）
Parallel実行での衝突解決開始時に、**対象 change_id を含む `ResolveStarted { change_id }` イベント**を送信する。

- `resolve_conflicts_with_retry` の呼び出し元（`src/parallel/mod.rs`）で、解決開始時に対象 change の `ResolveStarted` を送る
- `resolve_merges_with_retry` 内でも、各 change_id に対して `ResolveStarted` を送る
- TUI側は既存の `ResolveStarted` ハンドリングをそのまま活用（変更なし）

## 影響範囲（Impact）
- 影響する仕様:
  - `parallel-execution` (衝突解決時のイベント送信)
- 関連する実装領域:
  - `src/parallel/mod.rs` (`merge_and_resolve` / `resolve_merge_for_change`)
  - `src/parallel/conflict.rs` (`resolve_merges_with_retry`)
  - `src/tui/state/events.rs` (既存の `ResolveStarted` ハンドリング)

## 非ゴール（Non-Goals）
- `ConflictResolutionStarted` イベント自体の変更（破壊的変更を避ける）
- Serial実行での解決フロー変更
- TUI以外のモード（CLI `run`）での表示改善

## 受け入れ条件（Acceptance Criteria）
- Parallel実行でmerge衝突が発生し自動解決が開始されると、TUIの対象 change が `resolving` ステータスになる
- 解決完了/失敗時に、対応する `ResolveCompleted` / `ResolveFailed` が送信され、TUI表示が更新される
- 複数 change を順次マージする場合、各 change に対して適切にイベントが送信される
- 既存のテストが全て通る
