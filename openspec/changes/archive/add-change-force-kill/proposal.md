---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/changes/archive/2026-02-09-add-single-change-stop/proposal.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/web-monitoring/spec.md
  - src/tui/command_handlers.rs
  - src/tui/orchestrator.rs
  - src/web/api.rs
  - dashboard/src/components/ChangeRow.tsx
---

# Change: add-change-force-kill

**Change Type**: implementation

## Problem / Context
現状の change 単位停止は `stop-and-dequeue` という名前と UI を持っているが、実装実態は `dynamic_queue.mark_stopped(change_id)` を中心にした協調的キャンセルであり、実行中の agent command / child process を即時に kill することが保証されていない。

その結果、TUI と WebUI のどちらから停止しても、長時間ブロック中の apply / acceptance / archive / resolve が即座に止まらず、「止めたつもりなのにまだ動く」状態が起こりうる。tumf の要求は change 単位での強制 kill を TUI / WebUI の両方から提供することにある。

過去の `add-single-change-stop` 変更は TUI での単体停止要求と状態遷移を導入したが、Non-Goals として Web API からの単体停止を除外しており、また in-flight 実行プロセスを change ID から直接 kill する共通経路までは標準化していない。

## Proposed Solution
change ごとの実行ハンドルを backend 側で追跡し、change ID をキーに in-flight agent command を強制終了できる共通 force-kill 経路を追加する。

具体的には以下を行う。
- 実行中 change の process / cancellation handle を change ID に紐づけて登録する
- TUI では `Space` を queue 操作専用のまま維持し、active change の強制停止は `K` で確認モードに入り、`y` で確定する二段階操作にする
- TUI の change 単位停止コマンドは「単なる停止要求」ではなく force-kill を伴う stop-and-dequeue にする
- Web API の `POST /api/v1/projects/{project_id}/changes/{change_id}/stop-and-dequeue` を、running change に対して強制 kill を保証する契約へ更新する
- kill 完了後は対象 change のみ `not queued` / `selected=false` に戻し、他の queued changes は継続する
- kill 失敗時は change を勝手に dequeue 完了扱いにせず、エラーを返しログと UI に反映する
- TUI のキーヒントは active row で `K: kill`, 確認モードで `Y: confirm kill / N: cancel` を表示し、誤操作しやすい `Space` 停止を避ける
- WebUI でも active change の停止は即実行せず、Stop ボタン押下後に確認ダイアログを表示し、明示的な confirm 後にのみ force-kill API を呼ぶ

`K` を選ぶ理由は、既存のキーマップと衝突しにくく、`kill` の意味が明確で、`Space` に比べて誤爆しにくいためである。さらに `K -> y` の二段階にすることで、実行中 change の破壊的停止を明示的に確認できる。

## Acceptance Criteria
- TUI で active change に stop 操作を行うと、対象 change の in-flight agent command が強制終了される
- WebUI の active change 行から Stop 操作を行うと、確認ダイアログを経由したうえで同じ backend force-kill 経路が呼ばれる
- 強制停止完了後、対象 change のみ `not queued` になり `selected=false` へ戻る
- 他の queued / running changes は継続し、全体停止へ誤変換されない
- force-kill 対象が存在しない queued/not-queued change への stop-and-dequeue は安全に処理される
- kill に失敗した場合、API と TUI は失敗を観測でき、状態を誤って完了扱いにしない

## Out of Scope
- 新しい永続停止状態語彙（例: `force-stopping`）の追加
- proposal session 用 cancel UI / API の見直し
- 全体停止 (`/control/force-stop`) の意味変更
