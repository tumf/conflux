## 1. Implementation

- [ ] 1.1 stream-json textifier に「JSONとして認識できたが text を抽出できない場合は抑止する」ための判定を追加する（verification: unit tests cover parseable JSON non-text events are suppressed in textify mode）
- [ ] 1.2 `src/ai_command_runner.rs` の stdout ストリーミングで、非テキストイベントを出力しないように統合する（verification: targeted unit test or integration test asserts JSON lines are not forwarded when textify enabled）
- [ ] 1.3 既存の legacy streaming path (`src/agent/runner.rs`) でも同一の抑止挙動に揃える（verification: unit test for the legacy path behavior or shared helper test）
- [ ] 1.4 `stream_json_textify=false` の場合は従来通り「素通し」になることを確認する（verification: test asserts JSON line is forwarded unchanged when disabled）

## 2. Validation

- [ ] 2.1 `openspec validate update-stream-json-textify-suppress-nontext-events --strict --no-interactive`（verification: passes）

## Future Work

- デバッグ用途に非テキストイベントを要約表示するモードの追加（必要が出た場合）
