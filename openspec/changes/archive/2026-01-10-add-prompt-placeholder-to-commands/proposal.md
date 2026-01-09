# Proposal: Add {prompt} Placeholder to apply_command and archive_command

## Summary

`apply_command` と `archive_command` に `{prompt}` プレースホルダーを追加し、システムから追加の指示をコマンドに注入できるようにする。

## Background

現在の設定では:
- `apply_command` - `{change_id}` プレースホルダーのみサポート
- `archive_command` - `{change_id}` プレースホルダーのみサポート
- `analyze_command` - `{prompt}` プレースホルダーのみサポート

ユーザーの要望:
- `apply_command` に「スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。」といった追加指示を渡したい
- `archive_command` にも将来的な拡張のため `{prompt}` プレースホルダーを追加したい（現時点では空文字列）

## Proposed Changes

1. `apply_command` と `archive_command` の両方で `{change_id}` と `{prompt}` の両方のプレースホルダーをサポート
2. デフォルトの apply prompt を設定: `"スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。"`
3. デフォルトの archive prompt を空文字列に設定
4. テンプレートとドキュメントを更新

## Use Cases

### Apply Command with System Prompt

現在:
```
claude -p '/openspec:apply {change_id}'
```

変更後:
```
claude -p '/openspec:apply {change_id} {prompt}'
```

実行時の展開:
```
claude -p '/openspec:apply add-feature スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。'
```

### Archive Command (Empty Prompt)

```
claude -p '/openspec:archive {change_id} {prompt}'
```

実行時の展開（空 prompt）:
```
claude -p '/openspec:archive add-feature '
```

## Impact

- 既存の設定ファイルは `{prompt}` なしでも動作する（後方互換性）
- 新規テンプレートには `{prompt}` を含める
- ドキュメントを更新してプレースホルダーの使用方法を説明
