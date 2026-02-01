# Change: 変更一覧ログプレビューのUnicode安全な省略

## Why
TUIの変更一覧でログプレビューを省略する際、UTF-8の文字境界を壊すとpanicが発生します。日本語などのマルチバイト文字を含むログでも安定して表示できるようにします。

## What Changes
- 変更一覧のログプレビュー省略処理をUnicodeの文字境界を壊さない方式に変更する
- 省略処理の共通ヘルパーを追加し、表示幅ベースで安全に切り詰める
- 日本語を含むログプレビュー省略の回帰テストを追加する

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/render.rs, src/tui/utils.rs
