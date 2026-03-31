## Implementation Tasks

- [ ] `_validate_change_dir` の strict ブロック（262-271行目付近）を修正: `.no-delta` マーカーの存在を確認し、delta なしを許容するロジックを追加 (verification: `python3 scripts/cflx.py validate <test-change> --strict` が `.no-delta` 付き change で PASS)
- [ ] `.no-delta` と spec delta ディレクトリの共存を検出してエラーにするバリデーション追加 (verification: 共存時に validation error が出ること)
- [ ] `archive_change` の動作確認: `.no-delta` 付き change が archive 可能であること (verification: `python3 scripts/cflx.py archive <test-change> --yes` が成功)
- [ ] `_simulate_spec_promotion` が `specs/` 内にディレクトリなしの場合に空リストを返すことを確認 (verification: 既存テストが壊れないこと)
- [ ] ユニットテスト追加: `.no-delta` のみ / `.no-delta` + spec delta 共存 / どちらもなし の3パターン (verification: `pytest` または手動テスト)

## Future Work

- agent の archive プロンプトに `.no-delta` マーカーの作成ガイダンスを追加
- proposal 作成時に change_type に応じて `.no-delta` を自動生成する機能
