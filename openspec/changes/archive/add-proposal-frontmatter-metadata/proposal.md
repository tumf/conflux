# Change: proposal.md frontmatter metadata を追加する

**Change Type**: hybrid

## Problem / Context

- 現在の `openspec/changes/*/proposal.md` は本文セクション中心で、機械可読な frontmatter を正式には持っていない。
- 依存関係は本文の `## Dependencies` セクションから解析されており、proposal session の change 検出では先頭の `# ` タイトルだけを使っている。
- この状態では、priority・dependencies・references のような proposal metadata を安定して機械利用できず、proposal 作成規約と実装側の解釈が分離しやすい。
- ユーザー要望として、proposal frontmatter に優先度・依存関係・参照先を追加したい。参照先フィールド名は `references` を採用する。

## Proposed Solution

- `proposal.md` に YAML frontmatter を正式導入するが、frontmatter 自体は任意とし、既存の本文中心 proposal も引き続き有効にする。
- frontmatter では少なくとも `change_type`、`priority`、`dependencies`、`references` を記述可能にする。
- `priority` は `high | medium | low` の列挙値とする。
- `dependencies` は change id 配列とし、既存の本文 `## Dependencies` セクションより優先して解釈する。
- 後方互換のため、frontmatter に `dependencies` がない proposal では既存の `## Dependencies` セクション解析を継続する。
- `references` は文字列配列とし、関連ファイル・spec・change id など、実装やレビュー時に参照すべき対象を保持できるようにする。
- 既知キー以外の frontmatter key は proposal の読み取りを失敗させず、warning として報告する。
- proposal 作成規約、fixture、parser、proposal session の metadata 読み取りを順次整合させる。

## Acceptance Criteria

- `proposal.md` は YAML frontmatter を任意で持てることが proposal/spec 上で正式に定義される。
- `priority`、`dependencies`、`references` の意味と値形式が明文化される。
- `dependencies` は frontmatter を優先し、未指定時のみ本文 `## Dependencies` を読む後方互換ルールが明文化される。
- unknown frontmatter key は proposal を無効化せず、warning として扱うルールが明文化される。
- proposal 作成系の指示・fixture・検証観点が `references` を含む frontmatter 前提に更新される。
- proposal session / native proposal parser から、少なくとも title と新 metadata を安全に抽出できる実装タスクが定義される。

## Out of Scope

- `references` の内容を UI 上で高度表示・分類すること。
- `priority` による実行順序の自動最適化。
- 既存 archive proposal 全件への一括自動移行。
