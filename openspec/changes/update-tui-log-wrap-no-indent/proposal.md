# Change: Logsビューの折り返しで継続行をインデントしない

## Problem / Context

TUIのLogsビュー（ログパネル）では、長いログメッセージが表示幅を超えると複数行に折り返されます。
現状の折り返しは、2行目以降を `timestamp + header` 幅ぶん空白でインデントして表示します。

しかし、コマンド文字列のように「後半が重要」なログでは、継続行が大きく右に寄ることで視認性が下がります。
また、ユーザーの期待としては「継続行は行頭から続く（ヘッダ領域も折り返し幅に使う）」ほうが読みやすいケースがあります。

## Proposed Solution

Logsビューの折り返しルールを変更し、1行目のみ `timestamp + header` を表示し、2行目以降はインデントせずに表示幅全体を使ってメッセージを折り返して表示します。

- 1行目: `timestamp + header + message(先頭)`
- 2行目以降: `message(残り)` を行頭から表示（インデントしない）

## Acceptance Criteria

- Logsビューの長文ログが複数行になる場合、2行目以降は `timestamp + header` 相当の空白インデントを入れずに行頭から表示される。
- Logsビューのスクロール/表示範囲計算は折り返し後の表示行数ベースで維持され、長文ログの折り返しによって最新ログが画面外にならない。
- `timestamp` とログヘッダ（`[{change_id}:{operation}:{iteration}]` 等）の表示ルールは維持され、ヘッダは1行目にのみ表示される。
- 既存のユニコード境界（UTF-8 char boundary）安全性を維持し、折り返し処理でpanicしない。

## Out of Scope

- 変更一覧（Changes list）のログプレビュー表示の折り返し/トランケーション仕様は変更しない（引き続き単一行トランケーション）。
- 単語境界（スペース等）を考慮したスマート折り返しは行わない（現状の文字境界での分割を踏襲）。

## Impact

- Specs: `openspec/specs/tui-architecture/spec.md`
- Code (expected): `src/tui/render.rs`（Logsビューの折り返し関数・描画ロジック）、関連テスト
