# タスク: 設定ファイル読み込みエラーの堅牢化

- [x] **[確認]** 既存の設定ローダーのテストを実行し、Characterization tests がパスすることを確認する。
- [x] **[リファクタリング]** `src/config/mod.rs` の設定ロード導線を確認し、設定ファイル読み込み時のパース失敗が `Result` 経由でファイルパス付きエラーとして伝播するよう `src/config/load.rs` を改善する。
- [x] **[検証]** リファクタリング後に正常系テストがパスし、不正な設定ファイル指定時にパニックせず適切なエラー情報を返すことを確認する（`test_load_from_custom_path_returns_parse_error_with_path_context` を追加）。
