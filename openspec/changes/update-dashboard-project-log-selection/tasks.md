## Implementation Tasks

- [x] 1.1 グローバルログ表示要件を `openspec/changes/update-dashboard-project-log-selection/specs/server-mode-dashboard/spec.md` に追加する（verification: spec delta が未選択時の全体ログ表示と選択時のプロジェクト別表示を定義している）
- [x] 1.2 `dashboard/src/store/useAppStore.ts` のプロジェクト選択操作をトグル化し、選択解除時の関連 UI 状態の扱いを定義する（verification: reducer/unit test で同一 projectId の再選択時に `selectedProjectId` が `null` になることを確認する）
- [x] 1.3 `dashboard/src/App.tsx` と `dashboard/src/components/LogsPanel.tsx` で未選択時に全体ログ、選択時にプロジェクト別ログを描画する（verification: コンポーネントテストまたは store-driven test で両表示が切り替わることを確認する）
- [x] 1.4 `dashboard/src/components/ProjectCard.tsx` のクリック/キーボード操作をトグル選択に更新する（verification: Enter / Space / click の各操作で再選択時に解除されることを確認する）
- [x] 1.5 `dashboard/src/store/useAppStore.test.ts` と必要な dashboard テストを更新し、`npm run lint` と `npm test` または既存 dashboard テストコマンドで回帰確認する（verification: 対象テストと lint が成功する）

## Future Work

- 必要であれば全体ログ表示時のプロジェクト識別ラベル改善を別提案で検討する

## Acceptance #1 Failure Follow-up

- [ ] 受け入れレビュー前に変更をコミットまたは破棄し、git working tree を clean にする
