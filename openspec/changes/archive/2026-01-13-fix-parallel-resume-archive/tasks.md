## 1. 調査
- [x] 1.1 resume時のworkspace状態判定とmerge処理の流れを確認する

## 2. 実装
- [x] 2.1 アーカイブ済みworkspaceを検出してapply/archiveをスキップする
- [x] 2.2 tasks.md欠落時のapplyループ停止条件を追加する
- [x] 2.3 skip時にworkspaceのrevisionをmerge対象として扱う

## 3. テスト
- [x] 3.1 resumeでアーカイブ済みchangeを扱うテストを追加する
- [x] 3.2 cargo testで関連テストを実行する
