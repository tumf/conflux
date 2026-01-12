# Design: shlex によるサブコマンド全体のエスケープ強化

## Context

現在の `expand_*()` は手動エスケープに依存しており、特殊文字や改行の扱いが不完全です。加えて、テンプレート側にクォートが含まれるかどうかで安全性が左右されるため、一貫したルールが必要です。

本変更では `shlex::try_quote()` を基盤にして、サブコマンドすべてのプレースホルダー展開を統一します。

## Goals / Non-Goals

### Goals
- `expand_prompt()` / `expand_proposal()` / `expand_conflict_files()` を shlex ベースに統一
- 既存テンプレートの後方互換を維持
- 特殊文字・改行・マルチバイト文字の扱いを安定化

### Non-Goals
- Windows の `cmd` 向けクォート仕様を shlex と完全統一すること
- テンプレート記法の破壊的変更

## Decisions

### Decision 1: shlex をコアエスケープとして採用

- **理由**: POSIX シェルの引用符規則に準拠し、広く利用されている
- **影響**: 文字列は shlex が返す「安全なトークン」として扱う

### Decision 2: テンプレート互換性レイヤーを実装

既存テンプレートの多くは `"... '{prompt}'"` のようにプレースホルダーをクォートしている。

そのまま shlex を適用すると二重クォートになるため、以下の方針で互換を維持する。

- テンプレート内で `{prompt}` の前後が単一引用符の場合は「クォート済み扱い」とみなし、`shlex` の外側クォートを除去した内容を挿入する
- テンプレート側にクォートがない場合は `shlex` の完全な結果を挿入する

このロジックは `expand_prompt()`/`expand_proposal()`/`expand_conflict_files()` に共通化する。

### Decision 3: Windows は現行挙動を維持しつつ安全化

- Windows では `cmd /C` を使用するため、POSIX 前提の `shlex` は直接適用しない
- 互換性を優先し、Windows は現行の挙動を維持
- ただし、改行・NULL 文字など明確に危険な入力はサニタイズする

## Implementation Plan

1. `expand.rs` に共通の `escape_shell_token()` を追加
2. `{prompt}`, `{proposal}`, `{conflict_files}` に共通処理を適用
3. テンプレートのクォート有無を検出して適切に展開
4. 既定テンプレートとテストを更新

## Risks / Trade-offs

- **互換性検出の誤判定** → クォート有無の検出を厳密にし、テストで担保
- **Windows の安全性不足** → POSIX と同等の安全性は保証しないことを明記
- **期待値変更** → 既存テストの期待値を更新し、実行系テストで担保

## Open Questions

- Windows 用に将来 `windows-args` や専用エスケープを導入するか
- `{change_id}` に対しても shlex を適用するか（通常は安全だが念のため）
