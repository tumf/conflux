## Implementation Tasks

- [x] 1. `.pre-commit-config.yaml` の fix-up hook 設定を更新し、`dashboard/dist/assets/index-*.(js|css)` など publish 対象の committed generated assets が `end-of-file-fixer` で書き換えられないようにする (verification: integration - update `.pre-commit-config.yaml`, run `bash dashboard/build.sh && prek run --all-files`, then run `git status --porcelain dashboard/dist/assets` and confirm `dashboard/dist/assets/index-*.js` / `dashboard/dist/assets/index-*.css` are not modified by fixer hooks)
- [x] 2. 既存の generated-asset 除外方針と整合するように hook policy を最小範囲で整理し、large-file check など他 hook の意図を壊さないことを確認する (verification: integration - inspect `.pre-commit-config.yaml` and run the hook suite to confirm dashboard generated assets are excluded only where fix-up behavior is unsafe)
- [x] 3. `docs/guides/DEVELOPMENT.md` を更新し、dashboard dist assets が committed publish artifacts であり fixer hook 対象外であること、標準検証で dirty worktree を残さない期待挙動を明記する (verification: manual - inspect `docs/guides/DEVELOPMENT.md` and confirm it explicitly names `dashboard/dist/assets` as committed publish artifacts, documents the fixer-hook exclusion policy, and points developers to the standard validation path)
- [x] 4. publish-readiness regression を検証し、dashboard build 後の標準ローカル検証が generated asset fix-up で止まらないことを確認する (verification: integration - run `bash dashboard/build.sh`, then `pre-commit run --all-files` or `prek run --all-files`, and `make check` to verify the hook phase does not modify `dashboard/dist/assets/index-*` files)
- [x] 5. proposal delta と検証計画を strict validation で確認する (verification: integration - run `cflx openspec validate exclude-generated-dashboard-assets-from-fixers --strict --evidence warn`)

## Future Work

- `dashboard/dist/` 配下の他 committed generated files（`index.html`, `svg`, `debug-ws.js`）に同じ hook policy を広げる必要があるかの再評価
- generated artifact を Git 管理する配布戦略自体の見直し

## Acceptance #1 Failure Follow-up
- [x] `.pre-commit-config.yaml:8-13` では `end-of-file-fixer` の除外対象が `^dashboard/dist/assets/index-.*\.(js|css)$` のみで、実際に hook が書き換えた `graphify-out/GRAPH_REPORT.md`・`graphify-out/graph.html`・`graphify-out/graph.json` が未除外です。変更対象の dashboard asset 問題自体は回避できていますが、repository の実 commit path では別の generated artifact が同じ fixer で失敗する新規問題を導入/露呈しており、`make check`/archive 前検証が clean に完走できません (verification: inspect `.pre-commit-config.yaml:8-13` and confirm `end-of-file-fixer` exclude regex does not match `graphify-out/GRAPH_REPORT.md`, `graphify-out/graph.html`, or `graphify-out/graph.json`).
- [x] `bash dashboard/build.sh && prek run --all-files` の実行で `fix end of files` hook が失敗し、`graphify-out/GRAPH_REPORT.md`・`graphify-out/graph.html`・`graphify-out/graph.json` を自動修正して dirty worktree を残します（agent-exec job `584b8051494205adf09c0f9fffc9b837` の stdout 18-29 行、`git status --porcelain` で上記 3 ファイルが `M`）。archive 前の通常 commit 経路で `prek run --all-files` が blocker になるため、現状は archive commit readiness を満たしていません (verification: run `bash dashboard/build.sh && prek run --all-files`, then `git status --porcelain`, and confirm the hook modifies `graphify-out/GRAPH_REPORT.md`, `graphify-out/graph.html`, and `graphify-out/graph.json`).
