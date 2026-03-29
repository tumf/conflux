# Change: analyze フェーズで proposal frontmatter metadata を参照する

**Change Type**: hybrid

## Problem / Context

- `proposal.md` frontmatter の導入 proposal は別 change で進めているが、analyze フェーズがその metadata をどう使うかはまだ明文化されていない。
- 現行 analyze は proposal ファイルを読んで order / dependencies を返す前提だが、frontmatter が追加されても `dependencies`、`priority`、`references` をどの程度ハード制約・ソフトヒントとして扱うかが不明確だと、実装と期待がずれやすい。
- 特に `dependencies` は dependency graph に直接影響し、`priority` は order にのみ影響させるべきか、`references` は解析コンテキストへ渡す補助情報に留めるべきかを分離して定義する必要がある。
- ユーザー要望として、cflx の analyze フェーズで frontmatter がある場合はそれを参考にしてほしい。ただし metadata 定義 change とは別 change に分けたい。

## Dependencies

- `add-proposal-frontmatter-metadata`

## Proposed Solution

- analyze フェーズ専用の別 change として、proposal frontmatter metadata の analyze 利用規則を定義する。
- `dependencies` は frontmatter に存在する場合、analyze の dependency source として本文 `## Dependencies` より優先して使う。
- `dependencies` が frontmatter に存在しない場合のみ、既存の本文 `## Dependencies` fallback を使う。
- `priority` は hard dependency にはせず、依存関係を壊さない範囲で order のソフトヒントとして使う。
- `references` は hard dependency source にはせず、analyze プロンプトや解析コンテキストに「参照先」として渡す補助情報にする。
- unknown frontmatter key は analyze を失敗させず、metadata parser 側の warning を尊重したまま analyze を継続する。
- 実装タスクは `src/analyzer.rs` の prompt 構築と metadata 読み取り経路の接続に限定し、frontmatter 自体の定義や parser 基盤は別 change に委ねる。

## Acceptance Criteria

- analyze フェーズで、frontmatter `dependencies` がある proposal は本文 `## Dependencies` より frontmatter を優先して依存関係 source として扱うことが明文化される。
- frontmatter `dependencies` がない proposal では、既存どおり本文 `## Dependencies` fallback を使うことが明文化される。
- frontmatter `priority` は dependency graph には影響させず、order のソフトヒントとしてのみ使うことが明文化される。
- frontmatter `references` は hard dependency として扱わず、analyze の補助コンテキストとしてプロンプトに含めることが明文化される。
- unknown frontmatter key warning が存在しても analyze は継続できることが明文化される。
- `src/analyzer.rs` まわりの実装・テスト観点を含む tasks が定義される。

## Out of Scope

- `proposal.md` frontmatter の形式定義そのもの
- proposal session や Dashboard での metadata 表示
- `priority` を scheduler の強制ルールとして扱うこと
- analyzer 以外の apply / archive / resolve フェーズでの metadata 利用
