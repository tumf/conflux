## Context
- `jj_workspace.rs` と `parallel_executor.rs` で `jj` コマンド実行ロジックが重複している。
- `agent.rs` と `opencode.rs` に類似したプロセス実行・ストリーミング処理が存在する。
- `#[allow(dead_code)]` が広範囲に存在し、未使用コードの判断が困難。

## Goals / Non-Goals
- Goals:
  - 重複するコマンド実行処理を共通化して保守性を向上させる。
  - レガシー／未使用コードを整理し、不要な `#[allow(dead_code)]` を削減する。
  - 既存の挙動を変更せずにリファクタリングを進める。
- Non-Goals:
  - 仕様やユーザー向け機能の変更。
  - 新しいコマンドやUI機能の追加。

## Decisions
- Decision: `jj` 実行ヘルパーは `jj_workspace` と `parallel_executor` の双方から参照できる共通関数として切り出す。
- Decision: `opencode.rs` は利用状況がない場合に削除し、利用がある場合は `legacy` として明示的に隔離する。
- Alternatives considered: 現状維持（重複容認）は保守コストが高いため採用しない。

## Risks / Trade-offs
- 既存のエラーメッセージやログフォーマットが変わる可能性 → 既存のログ出力と互換性を確認する。
- レガシー削除による利用者影響 → 利用箇所の調査と明示的な移行方針を先に決定する。

## Migration Plan
1. 重複箇所と未使用コードの棚卸し
2. 共通ヘルパー導入 → 影響箇所の段階的移行
3. レガシー削除／隔離を実施
4. フォーマット・lint・テストで挙動を確認

## Open Questions
- `opencode.rs` を参照している外部利用者がいるか
- `jj` 実行ロジックの共通化で必要なオプション差分があるか
