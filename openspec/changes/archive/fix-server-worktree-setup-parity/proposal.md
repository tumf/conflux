# Change: server mode の worktree 作成で `.wt/setup` 実行を通常経路と一致させる

**Change Type**: implementation

## Why
`cflx server` のプロジェクト追加フローは worktree を作成するが、現行実装では通常の VCS worktree 作成経路と異なり `.wt/setup` が実行されない。

その結果、同じリポジトリでも「通常経路で作った worktree」と「server mode で作った worktree」で初期化結果が不一致になり、実装環境が欠落する。

## Proposed Solution
- server mode の `POST /api/v1/projects` における worktree 作成後に、リポジトリ直下 `repo_root/.wt/setup` を通常経路と同等条件で実行する
- `~/.wt/setup`（ホームディレクトリ直下）は引き続き参照しない
- `.wt/setup` 実行失敗時はプロジェクト追加を失敗として扱い、既存の rollback 方針に従って中途半端な登録状態を残さない
- server-mode spec を更新し、`~/.wt/setup` 非参照ポリシーと `repo_root/.wt/setup` 実行要件の境界を明示する

## Acceptance Criteria
1. server mode で project 追加時、`repo_root/.wt/setup` が存在すれば実行される。
2. `.wt/setup` 実行時、通常経路と同様に `ROOT_WORKTREE_PATH` が設定される。
3. `.wt/setup` が失敗した場合、API はエラーを返し、追加対象プロジェクトは registry に残らない。
4. server mode は引き続き `~/.wt/setup` を参照しない。

## Out of Scope
- `.wt/setup` のスクリプト内容そのものの標準化
- server mode 以外の worktree 実装の機能変更

## Impact
- Affected specs: `openspec/specs/server-mode/spec.md`
- Affected code: `src/server/api.rs`（必要に応じて共通化先の VCS helper も含む）
