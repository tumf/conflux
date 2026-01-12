# Change: CLI/TUI オーケストレーションロジックの統合

## Why

現在、CLI モード (`src/orchestrator.rs`) と TUI モード (`src/tui/orchestrator.rs`) で同じ目的のロジックが別々に実装されている。これは：

1. **バグの温床**: 一方を修正しても他方に適用されない（例：アーカイブパス検証バグ）
2. **機能の不一致**: CLI には LLM 分析があるが TUI にはない
3. **保守コストの増大**: 約2000行の重複コード
4. **テストの困難**: 同じロジックを2箇所でテストする必要

## 重複箇所の完全なリスト

### 1. アーカイブ処理
| CLI | TUI | 備考 |
|-----|-----|------|
| `archive_change()` L604-620 | `archive_single_change()` L35-200 | TUI版は検証ロジック追加だがパスにバグ |
| - | `archive_all_complete_changes()` L206-280 | TUI専用のバッチアーカイブ |

### 2. Apply 処理
| CLI | TUI | 備考 |
|-----|-----|------|
| `apply_change()` L587-602 | L530-640 (インライン) | フック呼び出しパターンが重複 |
| `run_apply()` 使用 | `run_apply_streaming()` 使用 | ストリーミングの有無のみ |

### 3. メインループ
| CLI | TUI | 備考 |
|-----|-----|------|
| `run()` L155-495 | `run_orchestrator()` L292-700 | ほぼ同じ構造 |
| max_iterations チェック L252-275 | max_iterations チェック L352-375 | 同一ロジック |
| on_start/on_finish フック | on_start フック（on_finish なし？） | 不一致 |

### 4. 変更選択ロジック
| CLI | TUI | 備考 |
|-----|-----|------|
| `select_next_change()` L499-535 | L445-470 (インライン) | CLI は LLM、TUI は進捗ベースのみ |
| `analyze_with_llm()` L537-555 | なし | **機能の不一致** |

### 5. 状態管理
| CLI | TUI | 備考 |
|-----|-----|------|
| `initial_change_ids: HashSet` | `pending_changes: HashSet` | 同じ目的、異なる変数名 |
| `completed_change_ids: HashSet` | `archived_changes: HashSet` | 同じ目的、異なる変数名 |
| `apply_counts: HashMap` | `apply_counts: HashMap` | 同一 |

### 6. フックコンテキスト構築
- 両方で `HookContext::new()` を繰り返し呼んでいる
- 同じパターンが10箇所以上で重複

### 7. Parallel 実行
| CLI | TUI | 備考 |
|-----|-----|------|
| `run_parallel()` L717-800 | `run_orchestrator_parallel()` L715-920 | ParallelRunService のラッパー |
| `run_parallel_dry_run()` L658-715 | なし | CLI専用 |

## What Changes

段階的なリファクタリングを提案：

### Phase 1: 共通ロジックの抽出
- `src/orchestration/` モジュールを新設
- アーカイブ処理、apply 処理、フックコンテキスト構築を共通化

### Phase 2: 状態管理の統一
- `OrchestratorState` トレイトまたは構造体で状態を抽象化
- CLI/TUI 両方から使用

### Phase 3: メインループの統一
- イベント駆動の共通ループを設計
- CLI は同期的に、TUI は非同期チャネル経由で使用

### Phase 4: 変更選択ロジックの統一
- TUI にも LLM 分析を追加（またはオプションとして）

## Impact

- Affected specs: code-maintenance
- Affected code:
  - 新規: `src/orchestration/` モジュール
  - 修正: `src/orchestrator.rs`, `src/tui/orchestrator.rs`
- **BREAKING**: なし（内部リファクタリング）
