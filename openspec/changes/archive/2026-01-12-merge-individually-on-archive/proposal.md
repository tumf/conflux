# 提案: Archive 完了時に個別にマージする

## Why

並列実行モードにおいて、グループ単位の一括マージは以下の問題を引き起こしている:

1. **詰まりによる全体停止**: グループ内の1つの変更が apply で詰まると、他の完了した変更も archive されずに残り続ける
2. **マージ未実行**: `final_revision` が揃わないため完了済み変更が本体ブランチに反映されない
3. **ユーザー体験の悪化**: 「archived になったのに not queued に戻る」という混乱が発生

実際のケース: `add-web-approval-api` の apply が詰まった際、他の3変更（`fix-propose-submit-crash`, `update-proposal-edit-status`, `refactor-serial-parallel-orchestration`）は完了しているが archive されない状態が発生。

この問題を解決するため、各変更が archive 完了した時点で**即座に個別マージ**を実行する方式に変更する。

## What Changes

**変更内容**:
- 並列実行の archive 完了後、各変更を**個別に**即座にマージする
- グループ単位の一括マージを廃止

**影響ファイル**:
- `src/parallel/mod.rs` (グループ単位マージ削除、個別マージ追加)
- `src/parallel/executor.rs` (archive後のマージタイミング調整)
- `src/events.rs` (MergeStarted/MergeCompleted イベント追加)

**実装詳細は `specs/parallel-execution/` の MODIFIED/REMOVED 要件を参照**
