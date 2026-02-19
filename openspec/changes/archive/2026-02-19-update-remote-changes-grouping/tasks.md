## 1. グルーピング用の描画データ生成

- [x] 1.1 `src/tui/render.rs` に remote change id から project/change 表示を抽出するヘルパーを追加する
  - 検証: `src/tui/render.rs` に `split_once("::")` と `rsplit_once('/')` を使う表示用関数が追加されている
- [x] 1.2 Changes 一覧用に header/change の行モデルを生成し、change_index→visual_index のマッピングを構築する
  - 検証: `render_changes_list_select` と `render_changes_list_running` 内で header 行が挿入され、`ListState.select` が visual index を参照している

## 2. Changes表示の更新

- [x] 2.1 Select モードで header 行を描画し、change 行には change_id のみを表示する
  - 検証: `src/tui/render.rs` の Select リスト生成で project header と change 行の描画が分岐している
- [x] 2.2 Running モードでも同等に header 行を描画し、ログプレビュー幅計算に change 表示幅を反映する
  - 検証: `render_changes_list_running` の幅計算が change 表示用文字列を参照している

## 3. カーソル選択の整合性

- [x] 3.1 見出し行が選択対象にならないよう、list_state の選択位置を change 行に対応させる
  - 検証: `render_changes_list_select`/`render_changes_list_running` で `ListState.select` が change 行の visual index を参照している
