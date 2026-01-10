# Proposal: Open Editor in Change Directory

## Summary

TUIの選択モードで、カーソル位置のchangeディレクトリを対象としてエディタ（$EDITOR）を起動できるようにする。

## Problem Statement

現在のTUIでは、changeの内容を確認・編集するために、別のターミナルを開いてchangeディレクトリに移動する必要がある。これは以下の点で不便：

1. ワークフローが中断される
2. ディレクトリパスを手動で入力する必要がある
3. 複数のchangeを確認する際に繰り返し同じ操作が必要

## Proposed Solution

TUIの選択モードで `e` キーを押すと、カーソル位置のchangeディレクトリ（`openspec/changes/{change_id}/`）を作業ディレクトリとして `$EDITOR` を起動する。

### User Flow

1. ユーザーがTUIで変更一覧を確認
2. 確認・編集したいchangeにカーソルを移動
3. `e` キーを押下
4. TUIが一時停止し、`$EDITOR` が起動
5. エディタ終了後、TUIが復帰

### Key Binding

| Key | Action |
|-----|--------|
| `e` | カーソル位置のchangeディレクトリでエディタを起動 |

### Environment Variable

- `$EDITOR` 環境変数を使用
- 未設定の場合は警告メッセージを表示

## Scope

- TUI選択モードのみ（実行モードでは無効）
- 単一changeのみ対象（複数選択時は最初のもの）

## Out of Scope

- エディタ起動中のTUI更新
- エディタの種類別の特殊処理
- 特定ファイル（proposal.md等）を直接開く機能
