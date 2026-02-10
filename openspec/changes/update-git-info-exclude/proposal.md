# Change: .git/info/exclude に approved パスを自動追加

## Why
`openspec/changes/*/approved` がローカルで生成される場合、未追跡ファイルとして扱われると Git 実行前チェックが誤検知するため、安定して除外できる状態にする必要があります。

## What Changes
- `.git/info/exclude` に `openspec/changes/*/approved` が無い場合は自動で追加する
- 未追跡ファイル判定で `.gitignore` と `.git/info/exclude` の両方を反映する

## Impact
- Affected specs: `openspec/specs/cli/spec.md`
- Affected code: Git バックエンドの未追跡ファイル判定と初期化処理
