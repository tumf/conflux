## 1. Implementation
- [x] 1.1 依存分析入力に in-flight 変更を含めるための入力構造を追加する（検証: 分析関数に queued と in-flight の両方が渡されることを src/parallel/mod.rs で確認）
- [x] 1.2 LLM 依存分析プロンプトに「実行中の変更一覧」と「選択対象外で依存のみとして扱う」指示を追加する（検証: プロンプト生成箇所の出力に executing 節が含まれることを確認）
- [x] 1.3 依存関係の解釈が in-flight を含む前提で動作することを確認するためのテストを追加・更新する（検証: `cargo test parallel` が通ること）

## 2. Validation
- [x] 2.1 `npx @fission-ai/openspec@latest validate update-parallel-analysis-inflight-deps --strict --no-interactive`
