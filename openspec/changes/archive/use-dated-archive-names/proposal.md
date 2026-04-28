---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs
  - src/execution/archive.rs
  - src/tui/utils.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: standardize dated archive directory names

**Change Type**: implementation

## Premise / Context

- ユーザ要求は、archive 時の保存先を OpenSpec オリジナル互換の `YYYY-MM-DD-<change_id>` 形式へ標準化することだった。
- 現行の native archive 実装は `src/openspec_cmd.rs` で `openspec/changes/archive/<change_id>` へ移動しており、生成側はまだ日付付き標準になっていない。
- 一方で既存実装と canonical spec は、archive 解決・検証の読み取り側では direct match と date-prefixed match の両対応をすでに要求している。
- そのため現在は「読む側は両対応、書く側は非日付付き」というねじれがあり、標準 archive 形式が一貫していない。

## Problem / Context

archive 実行時の保存先命名が非日付付きのままだと、OpenSpec 互換の履歴表現と揃わず、archive 生成結果が repository 内で一貫しない。既存の TUI / workspace state / archive verification は date-prefixed archive entry を許容しているため、生成側だけが旧形式のままだと「標準形式」が不明確なまま残る。

この不整合は、archive 出力メッセージ、詳細解決、resume 判定、worktree 側の archive 確認の理解コストを上げる。今後の archive 系修正でも direct / dated の混在を前提に毎回判断が必要になり、保守性が下がる。

## Proposed Solution

native `cflx openspec archive` の保存先命名を `openspec/changes/archive/YYYY-MM-DD-<change_id>` に標準化する。

- archive 実行日のローカル日付を使って dated archive directory 名を生成する
- 新規 archive 生成は常に dated 形式を使う
- 既存 repository 互換のため、archive 解決・検証は direct match (`<change_id>`) と dated match (`<date>-<change_id>`) の両対応を維持する
- archive 成功メッセージは実際の dated path を返す
- 同日同 change の archive 先が既に存在する場合は明示エラーにする

## Acceptance Criteria

- `cflx openspec archive <change-id>` が成功したとき、change は `openspec/changes/archive/YYYY-MM-DD-<change-id>` に移動される
- archive 成功メッセージは `openspec/changes/archive/YYYY-MM-DD-<change-id>` を表示する
- archive 完了検証は、active change directory が消えている限り、direct archive entry と dated archive entry のどちらでも archive 完了として扱う
- archived change 解決は direct archive entry と dated archive entry の両方を同一 change として扱い続ける
- 同日の dated archive destination が既に存在する場合、archive は別名へ自動退避せず明示エラーを返す

## Explicit Completion Conditions

- `src/openspec_cmd.rs` の native archive 実装が dated archive destination を生成し、成功メッセージも同じ実パスを返す
- `src/openspec_cmd.rs` の change 解決テストに、dated archive entry を `show` で解決できるケースが追加または更新されている
- `src/execution/archive.rs` と関連 archive state 判定の既存両対応仕様を回帰させないテストが維持または追加されている
- `src/tui/utils.rs` の archived change 解決が dated entry を継続して開けることを示すテストが維持または追加されている
- proposal delta が strict validation を通過し、archive 命名変更を表す canonical spec 更新内容が明示されている

## Out of Scope

- proposal change ID 自体に日付を付ける変更
- 既存 archive directory を一括 rename する migration
- archive 日付のタイムゾーン設定やユーザ設定化
