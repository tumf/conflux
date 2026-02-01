## 1. プロンプト更新
- [ ] 1.1 `cflx-accept` にサブエージェント分割の手順を追加する（親が統合し `ACCEPTANCE:` を 1 回だけ出力すること、子は最終判定を出力しないことを明記する）
  - 検証: `.opencode/commands/cflx-accept.md` にサブエージェント手順と出力ルールが記載されている
- [ ] 1.2 サブエージェントの出力形式を統合しやすい構造（例: JSON もしくは見出し+根拠の箇条書き）として指示する
  - 検証: `.opencode/commands/cflx-accept.md` に具体的な出力フォーマット指示がある
- [ ] 1.3 サブエージェント利用不可時のフォールバック（逐次チェック）を明記する
  - 検証: `.opencode/commands/cflx-accept.md` にフォールバック手順が記載されている
- [ ] 1.4 スコープ制約（change_id/paths 以外はレビューしない）をサブエージェントにも適用する旨を明記する
  - 検証: `.opencode/commands/cflx-accept.md` にスコープ制約の再掲がある
