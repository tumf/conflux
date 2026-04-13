## Specification Tasks

- [ ] 1. 実装と不一致の旧要件 B (L31-62) を削除
  - Expected canonical result: 実装に一致する `local_sha_for_push` vs `remote_sha_for_push`（どちらも post-pull）比較ルール 1 つだけが canonical として残る
  - verification: manual — `src/server/api/git_sync.rs::plan_sync` (L181-188) と canonical spec のルール記述が一致していること
- [ ] 2. 要件 A 本文に `resolve_command` が AI エージェントを起動する点と事前判断の MUST 条項を追加
  - Expected canonical result: 「push 試行失敗からの事後検知ではなく SHA 比較で事前判断する」が MUST で明記される
  - verification: integration — `openspec validate git-sync --strict` が通過
- [ ] 3. Scenario `resolve_command invocation is decided before agent startup` を追加
  - Expected canonical result: エージェント起動前に SHA 比較ルールを評価する Scenario が存在する
  - verification: integration — `openspec validate git-sync --strict` が通過
- [ ] 4. 実装参照（`src/server/api/git_sync.rs` L181-188 / L491-499、`src/config/types.rs` L215-216 / L457-502）と推奨設定（トップレベル `resolve_command` 必須、`server.resolve_command` 廃止、冪等性要件）を Requirement 本文に追加
  - Expected canonical result: canonical spec を読むだけで実装エントリポイントと必須設定が特定できる
  - verification: manual — spec 記載のパス/行が `git grep` で到達可能であること
- [ ] 5. `bare repo is newly cloned (first sync)` Scenario を実装ルール（両 SHA が非空で一致した場合のみ skip）に合わせて書き直す
  - Expected canonical result: 初回クローン時の挙動が実装と矛盾しない（pre-pull SHA を空として扱うという旧 B 依存の記述を除去）
  - verification: manual — Scenario 記述が `plan_sync` のルールで再現可能であること
- [ ] 6. `## Purpose` セクションを追加
  - Expected canonical result: Purpose + Requirements の標準構造
  - verification: integration — `openspec validate git-sync --strict` が通過

## Future Work

- なし（実装変更は不要）
