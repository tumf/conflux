## 1. 実装
- [x] 1.1 依存関係分析のレスポンス形式を `order` に更新し、パーサ検証を追加する
- [x] 1.2 依存関係分析のプロンプトを `order` 形式に変更し、出力例を更新する
- [x] 1.3 再分析ループを 10 秒間隔のタイマー駆動に変更する
- [x] 1.4 キュー追加/削除時に 10 秒タイマーをリセットする
- [x] 1.5 空きスロット数を計算し、`order` から依存関係が解決済みの change を空き数分起動する
- [x] 1.6 依存解決後の実行開始時点で worktree を新規作成し、既存 worktree があれば再作成する
- [x] 1.7 group イベントの出力と CLI ログを新フローに合わせて整理する

## 2. 検証
- [x] 2.1 openspec の変更検証を実行する: `npx @fission-ai/openspec@latest validate update-parallel-reanalysis-order --strict`
