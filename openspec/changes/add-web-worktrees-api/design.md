## Context
TUIのWorktrees Viewではworktree一覧、衝突/先行コミット有無、削除、マージ、worktree_command実行が可能だが、Web UI/APIでは同等の取得/操作ができない。Web監視は既にWorktreeInfoを保持できる設計のため、API/画面の不足を補えば同一の運用が可能になる。

## Goals / Non-Goals
- Goals:
  - Web UI/APIでTUI Worktrees Viewと同等の情報取得と操作を提供する
  - TUIと同じガード/制約で安全に操作できる
  - 状態更新はWebSocket/REST双方で一貫させる
- Non-Goals:
  - Webからのエディタ起動やローカルシェル起動
  - worktree命名規則の変更や高度なgit操作の追加

## Decisions
- Decision: `/api/worktrees`配下に一覧/再取得/作成/削除/マージ/コマンド実行のREST APIを追加する
  - TUIのキー操作（+ / D / M / Enter）と対応関係を明確にする
- Decision: `WorktreeInfo`の型をWeb APIのレスポンス契約に採用する
  - 既存のserde導出を活用し、TUI/Webでデータ語彙を一致させる
- Decision: TUIで使用しているworktree取得・衝突/先行チェックを共有ロジック化しWebからも利用する
  - API/画面の表示を一致させるため
- Decision: 操作結果は`WorktreesRefreshed`と`BranchMerge*`をWebStateに反映し、WebSocketで通知する
  - RESTとWebSocketの整合性を維持するため
- Decision: `worktree_command`未設定/非Git環境では作成・コマンド実行を拒否する
  - TUIの挙動と安全性に合わせるため

## Risks / Trade-offs
- Git操作がAPI経由で実行されるため、誤操作時の影響が大きい
  - Mitigation: TUIと同一のガード（main除外、処理中change紐付けの禁止、衝突/先行チェック）を適用し、409で拒否する
- worktree再取得/衝突チェックのコスト増
  - Mitigation: 明示的なrefreshエンドポイントと、必要時のみの再計算に限定する

## Migration Plan
- 追加のみで既存UI/APIを壊さないため、移行は不要

## Open Questions
- なし
