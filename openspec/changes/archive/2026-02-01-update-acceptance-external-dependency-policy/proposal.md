# 提案: 外部依存ポリシーの統一（Mock-first）

## 概要

acceptance / apply / proposal の各プロンプトにおける「外部依存（APIキー欠如、HIL、外部システム等）」の扱いが不統一なため、実行が CONTINUE ループに陥ったり、意図せず本番用の鍵を要求したりする問題が発生しています。
本提案では、AI が単独で解決・検証できない要件を一貫して「外部依存」と分類し、**モック/スタブ/フィクスチャを優先して自己完結に検証できる状態へ寄せる**方針を、プロンプトと仕様の両方に明文化します。

## 背景（問題）

- 外部依存の扱いがステージごとに異なり、acceptance が「鍵がないので CONTINUE」や「本番鍵を要求」などの挙動に流れやすい
- apply が外部依存を Future Work に逃がす/逃がさないの判断が揺れ、結果として acceptance が検証不能のまま回り続ける
- 「AI で完結できない」要件の扱いが曖昧で、モックで解決できるのに放置される

## 方針（共通スタンス）

- **AI が単独で解決・検証できない要件は外部依存**として扱う
- 外部依存が **モック/スタブ/フィクスチャ**で代替可能なら、それを実装して **外部資格情報なしで検証可能**にしなければならない
- 例外として、**真に非モック可能**な外部依存のみ Out of Scope / Future Work に移動する（チェックボックスは付けない）
- **秘密情報の欠如は CONTINUE の理由にしない**。代替（モック等）を実装するか、非モック可能として Out of Scope 化するために、FAIL として具体的な follow-up タスクへ落とし込む

## 変更内容

- `src/config/defaults.rs` の `ACCEPTANCE_SYSTEM_PROMPT` を更新し、上記スタンス（mock-first / 非モック可能は Out of Scope / missing secret は FAIL）を明記する
- OpenCode のローカルコマンドプロンプトを更新し、proposal/apply ステージでも同じ分類・優先順位で行動するよう統一する
  - `~/.config/opencode/command/cflx-proposal.md`
  - `~/.config/opencode/command/cflx-apply.md`
- OpenSpec の能力仕様（`agent-prompts`）にポリシーを追加し、仕様として追跡可能にする
- 可能な範囲で最小のテスト（プロンプト文言の生成/含有チェック等）を追加し、回帰を防ぐ

## 影響範囲

- `src/config/defaults.rs`（acceptance システムプロンプトの文言）
- `~/.config/opencode/command/cflx-proposal.md`（proposal ステージの行動指針）
- `~/.config/opencode/command/cflx-apply.md`（apply ステージの行動指針）
- `openspec/specs/agent-prompts/spec.md`（仕様の更新は changes の delta で提案）

## リスク / 注意点

- 文言変更は挙動に直結するため、テストで最低限の期待（重要な禁止/必須フレーズ）を固定する
- `~/.config/opencode/command/*` はローカル環境依存のため、運用上「どの環境に適用するか」を明確にする

## 非目標

- 本番の外部サービスへ接続して検証する運用を導入しない
- 秘密情報をリポジトリへ追加/コミットしない
