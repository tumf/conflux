# Tasks

## Implementation Tasks

- [x] `src/analyzer.rs` の `build_parallelization_prompt()` メソッドを更新
  - change リストを `[x]` マーク付きで表示
  - ファイルパス (`openspec/changes/{id}/`) を明示
  - プロンプトに「`[x]` マークの change のみ分析する」指示を追加

- [x] プロンプトテンプレートの更新
  - 「Read the proposal files for these changes」を「Analyze these selected changes (marked with [x])」に変更
  - ファイルパス形式の説明を追加

## Testing Tasks

- [x] ユニットテスト追加: `test_build_prompt_with_selected_markers`
  - 選択済みと未選択が混在する change リストでプロンプト生成
  - `[x]` マーカーが正しく出力されることを確認
  - ファイルパスが正しく含まれることを確認
  - 追加テスト: `test_build_prompt_all_selected`, `test_build_prompt_none_selected`

- [x] 統合テスト: 既存の `analyze_groups` テストケースで動作確認
  - 生成されたプロンプトが正しい形式であることを確認
  - 既存の分析ロジックに影響がないことを確認（全572テスト通過）

## Documentation Tasks

- [x] `src/analyzer.rs` のドキュメントコメント更新
  - `build_parallelization_prompt()` のdocstringに新フォーマットを記載

## Validation Tasks

- [x] `cargo fmt` でフォーマット確認
- [x] `cargo clippy` でリント確認
- [x] `cargo test` で全テスト通過確認（572テスト全て成功）
