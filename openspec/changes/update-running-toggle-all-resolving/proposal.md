# Change: Allow bulk mark toggle during running resolve

**Change Type**: implementation

## Problem/Context
- 現状の TUI では `x: toggle all` が Select/Stopped モード限定で、Running 中は無効。
- `resolving` 中は Running モードのため、他 change の実行マーク（selected）を一括で切り替えられない。
- 一方で Space による個別トグルは Running 中も動作するため、操作性に一貫性がない。

## Proposed Solution
- Running モードでも `x` を有効化し、active 以外の change に対して一括トグルを実行する。
- 一括トグル対象のルールを明確化する:
  - `NotQueued` / `Queued`: selected を一括トグル可能
  - `MergeWait` / `ResolveWait`: queue_status と DynamicQueue は変更せず、selected のみ一括トグル
  - `Applying` / `Accepting` / `Archiving` / `Resolving`: 一括トグル対象外
- キーヒント要件を更新し、Running モードでも「対象が存在する場合」は `x: toggle all` を表示可能にする。

## Acceptance Criteria
1. Running モードかつ `resolving` 中でも、active 以外の change に対して `x` で selected の一括トグルができる。
2. Running モードで `x` を押しても、active change の queue_status は変更されず停止要求も発行されない。
3. Running モードで `x` を押しても、`MergeWait` / `ResolveWait` の queue_status と DynamicQueue は変更されない。
4. キーヒントは Running モードで一括トグル可能な対象がある場合に `x: toggle all` を表示し、対象がない場合は表示しない。
5. 関連ユニットテスト（`src/tui/state.rs`, `src/tui/render.rs`, 必要に応じて `src/tui/key_handlers.rs`）が追加/更新され、回帰を防止できる。

## Out of Scope
- Worktrees ビューでの一括操作ルール変更
- `Space` キーの既存挙動変更
- resolve queue のアルゴリズム変更（本提案は一括マーク操作に限定）
