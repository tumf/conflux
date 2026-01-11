# Change: 仕様シナリオに対応する不足テストを追加

## Why

現在の `docs/test-coverage-mapping.md` は cli と configuration 仕様のみをカバーしており、hooks、parallel-execution、tui-editor、workspace-cleanup などの仕様シナリオが未マッピングである。testing spec に従い、全仕様シナリオがテストでカバーされるべきである。

## What Changes

### 分析対象（未マッピングの仕様）
| Spec | シナリオ数 | 現状 |
|------|----------|------|
| hooks | 34 | 9テストのみ（`src/hooks.rs`） |
| parallel-execution | 29 | E2Eテストでカバー（部分的） |
| tui-editor | 23 | 未確認 |
| workspace-cleanup | 7 | 未確認 |
| tui-key-hints | 8 | 未確認 |
| tui-architecture | 7 | 未確認 |

### 実施内容
1. 全仕様のシナリオとテストのマッピングを更新
2. 不足テストを追加（ユニットテストで表現可能なもの）
3. UIレンダリングテストは除外（スナップショットテスト等で別途対応）

## Impact

- Affected specs: testing
- Affected code: `src/hooks.rs`, `src/approval.rs`, `src/vcs_backend.rs`, `src/git_workspace.rs`, `src/jj_workspace.rs`, `src/tui/`
- テスト数増加（推定 +30-50 テスト）
