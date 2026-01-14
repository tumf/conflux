## Context
parallel モードでは各 change が worktree で apply/archive された後、base ブランチへ逐次統合（resolve_command によるマージ）される。
一方で base が dirty の状態（未コミット変更やマージ進行中）では、マージの前提が崩れており失敗が発生しやすい。

## Goals / Non-Goals
- Goals:
  - base dirty を検知した場合に、安全にマージを延期できること
  - ユーザに「マージ待ち」を明示し、base cleanup 後に手動で解決できること（TUI の `M`）
  - 依存関係を尊重しつつ、依存しない change の処理は継続すること
- Non-Goals:
  - base が clean になるまで自動的にマージを再開/リトライすること
  - resolve_command の手順（逐次マージ/メッセージ規約）を変更すること

## Decisions
- Decision: base dirty の場合、個別マージは「非致命の延期」として扱う
  - 既存の挙動（エラーで中断）ではユーザ操作（cleanup）で復旧できるケースでも不必要に失敗扱いになる
- Decision: `MergeWait` 状態を導入し、対象 change の worktree を保持する
  - これによりユーザは base cleanup 後に同一 worktree を使って解決できる
- Decision: `M` は選択中 change のみを解決する単発操作とする
  - 自動再開（マージ/実行）は行わず、ユーザが明示的に実行を再開する

## State Model
- `MergeWait`: archive は完了しているが base への統合が保留されている状態
- `BlockedByMergeWait`（内部概念）: `MergeWait` の change に依存するため、今回の run では実行しないがキューには残る状態

## Event Model
- `ExecutionEvent::MergeDeferred { change_id, reason }`
  - reason には base の dirty 判定根拠（例: `git status --porcelain` の出力や `MERGE_HEAD` の存在）を含める
- 既存の完了イベント（AllCompleted/Stopped）は、merge 待ちの場合に誤解を生まないよう停止理由を区別する

## Dependency Handling
- 依存解決は「未統合の change がある状態では、その依存先を実行しない」という安全側の制約を採用する
- `MergeWait` の change に依存しない queued change は通常通り実行を継続する
- `MergeWait` の change に依存する queued change はキューに残し、ユーザが `M` で解決した後に再度 run されたときに実行される

## Alternatives Considered
- Alternative: base dirty を検知した時点で全体を即停止
  - 単純だが、独立した change の進行まで止まるため採用しない
- Alternative: base が clean になったら自動的にマージを再開
  - ユーザ要件（自動再開しない）に反するため採用しない
- Alternative: base dirty を merge 失敗（fatal）扱い
  - 復旧可能なケースで不必要に失敗扱いになるため採用しない

## Risks / Trade-offs
- `MergeWait` を導入すると、ユーザが手動で解決するまでキューの一部が進まない
  - ただし安全性（不完全な統合で依存 change を進めない）を優先する

## Migration Plan
- 既存の挙動を置き換える形で導入する（フラグ追加はしない）
- 既存の resolve_command 契約は維持する

## Open Questions
- なし（自動再開しない、M は選択中のみ、という要件は固定）
