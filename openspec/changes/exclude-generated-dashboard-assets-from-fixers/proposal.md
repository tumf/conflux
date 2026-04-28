---
change_type: implementation
priority: medium
dependencies: []
references:
  - .pre-commit-config.yaml
  - Cargo.toml
  - dashboard/build.sh
  - docs/guides/DEVELOPMENT.md
  - openspec/specs/release-workflow/spec.md
---

# Change: exclude generated dashboard assets from fixer hooks

**Change Type**: implementation

## Premise / Context

- 現セッションでは `make publish` 失敗の調査から、実際の blocking point が `make check -> pre-commit` であることを確認した。
- `dashboard/dist/**` は `Cargo.toml` に publish 対象として含まれており、リポジトリ上では配布に必要なコミット済み生成物として扱われている。
- `.pre-commit-config.yaml` は `check-added-large-files` にだけ `dashboard/dist/assets/index-.*\.js` 除外を持つが、`end-of-file-fixer` には同等の除外がなく、生成 JS が hook によって毎回書き換えられうる。
- ユーザ報告では `end-of-file-fixer` が `dashboard/dist/assets/index-HFOU60M1.js` を修正し、生成物が毎回 pre-commit に引っかかるため publish readiness が崩れている。
- 既存仕様では `make check` が pre-commit checks を含む包括的ローカル検証コマンドとして定義されているため、標準検証経路が generated asset の自動修正で dirty になる挙動は改善対象になる。

## Problem / Context

Conflux は dashboard build の出力を `dashboard/dist/**` に保持し、それを publish/package 対象としてコミットしている。一方で現在の pre-commit 設定は、その生成 asset に対して `end-of-file-fixer` のような fix-up hook を通常ソースと同様に適用してしまう。

この状態だと、開発者が dashboard をビルドした直後や publish 前に標準検証を実行すると、hook が生成ファイルを書き換えて停止し、`make check` と `make publish` の readiness が毎回 dirty worktree に左右される。これは「標準ローカル検証を通したら生成物が勝手に変わる」という運用不安定性を生み、配布用 artifact をコミット管理している repo 方針とも噛み合わない。

## Proposed Solution

pre-commit / prek 互換設定を調整し、コミット済み dashboard generated assets を fix-up hook の対象から外す。

- `dashboard/dist/assets/index-*.js` を少なくとも `end-of-file-fixer` の対象外にする
- 再発防止のため、同じ generated asset 群に対する fix-up hook policy を必要最小限の範囲で揃える
- repository-standard validation (`make check`, `pre-commit run --all-files`, `prek run --all-files`) が generated asset を書き換えずに pass/fail だけを返すようにする
- 開発者向けガイドに「dashboard dist assets は publish 用の committed generated files であり、fix-up hook の対象外である」ことを明記する

## Acceptance Criteria

- `dashboard/dist/assets/index-*.js` のような committed dashboard generated assets は `end-of-file-fixer` 実行後も hook によって書き換えられない
- `check-added-large-files` など既存の generated-asset 除外方針と矛盾しない形で、dashboard asset hook policy が整理されている
- dashboard build 実行後に `pre-commit run --all-files` または documented equivalent を実行しても、generated asset 自体の fix-up による失敗で停止しない
- `make check` の pre-commit phase は generated dashboard asset の自動修正を原因に dirty worktree を残さない
- 開発ガイドに dashboard generated asset と hook policy の扱いが追記されている

## Explicit Completion Conditions

- `.pre-commit-config.yaml` に dashboard generated assets を fix-up hook から除外する設定が追加または整理されている
- generated asset を rebuild した状態で `pre-commit run --all-files` または `prek run --all-files` を実行し、dashboard asset が追加修正されないことを示す検証手順が用意されている
- `docs/guides/DEVELOPMENT.md` に generated dashboard assets と hook policy の説明が追加されている
- strict validation を通る spec delta が追加されている
- 提案された検証コマンドが、stub/no-op ではなく実際に「generated asset が hook で書き換わらない」ことを確認する内容になっている

## Out of Scope

- `dashboard/dist/` 全体の Git 管理方針を廃止すること
- Vite build 出力そのものの改行や minify 方式を変更すること
- dashboard generated asset 以外の一般ソースファイルに対する fixer policy の全面見直し
