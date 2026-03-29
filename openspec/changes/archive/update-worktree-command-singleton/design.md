## Context
- server-mode は `src/server/api.rs` の REST / WebSocket と `dashboard/` の React UI で構成される
- 現状の `git/sync` は project 単位ロックで直列化されるが、busy 状態を dashboard に公開していない
- `git/sync` の resolve_command は exit code のみを扱い、stdout/stderr を project log に流していない
- ユーザー要件は「Sync 状態のリロード耐性」「プロジェクトごとの Sync」「同一 root 競合の防止」「busy 時は即時 409」

## Goals / Non-Goals
- Goals:
  - base を含む worktree root 単位でコマンド実行をシングルトン化する
  - busy root の状態を WebUI へ配信し、リロード後も復元可能にする
  - Sync の内部 resolve 出力を既存の project log ストリームへ統合する
- Non-Goals:
  - サーバー再起動をまたぐジョブ永続化
  - busy root 要求の待機キュー化
  - 既存オーケストレーション全体をジョブシステムへ置き換えること

## Decisions
- Decision: 排他単位は project_id ではなく worktree root の実パス相当識別子とする
  - Why: base と各 worktree は別の実行対象であり、同一 root への競合のみを正確に防ぎたい
- Decision: busy root への新規要求は待機させず即時 `409 Conflict` を返す
  - Why: hidden queue を作らず、WebUI の disable 状態と API の挙動を一致させるため
- Decision: active command はサーバーメモリ上の真実源とし、`full_state` に含めて dashboard が参照する
  - Why: ブラウザリロードに耐えつつ、サーバー再起動時の複雑な再構築を避けられる
- Decision: Sync の resolve_command 出力は既存 `RemoteLogEntry` ストリームで project log に送る
  - Why: 新しいログ経路を増やさず、LogsPanel でそのまま可視化できるため

## Risks / Trade-offs
- root ごとの active command 管理を複数ハンドラへ適用するため、対象 API の洗い出し漏れがあると排他が不完全になる
- `409 Conflict` を返す設計は簡潔だが、自動リトライを期待するクライアントには別途ハンドリングが必要になる
- active command を `full_state` に載せることで DTO が拡張されるため、dashboard と server の整合更新が必須になる

## Migration Plan
1. server-mode の状態に active command レジストリを追加する
2. `git/sync` から root 単位ガードを適用し、ログ配信を追加する
3. 他の worktree 変更系 API に同じガードを適用する
4. `full_state` / dashboard state を拡張し UI disable を active command 駆動へ切り替える
5. 回帰テストと strict validate を通す

## Open Questions
- active command DTO に経過時間や表示用ラベルをどこまで含めるか
- apply 系の内部実行を server-mode API レベルでどこまで active command に反映させるか
