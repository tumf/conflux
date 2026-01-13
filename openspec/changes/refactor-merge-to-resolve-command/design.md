## Context
現状の並列実行（Git worktree）では、マージ処理はオーケストレータが `git merge` を実行し、衝突時のみ `resolve_command`（LLM）を呼び出す。
この構造では、
- 「LLM にマージ完了（git add/commit）まで担当させたい」
- 「マージコミットに change_id を必ず含めたい」
- 「pre-commit がファイルを修正してコミットを止めた場合でも自動収束させたい」
という要件を満たしにくい。

## Goals
- `resolve_command` を Git マージの主体として扱い、衝突有無に関わらず「逐次マージ→マージコミット作成」まで完了させる
- マージコミットメッセージに、対象ブランチに対応する `change_id` を含める
- pre-commit による自動修正でコミットが中断される場合でも、再ステージ→再コミットで収束させる
- オーケストレータは成功検証を行い、マージコミット作成成功を確実に検出できる

## Non-Goals
- octopus merge の採用（今回は逐次マージを維持）
- リモート push / PR 作成などの VCS 操作
- change_id 抽出規則の大幅な変更（現行の worktree 名規約に依存）

## Decisions
### Decision: マージの書き込み系 Git 操作を resolve_command に委譲
- `git merge` / `git add` / `git commit` によりマージを完了させる責務を resolve_command に集約する
- オーケストレータはマージ結果の検証（例: HEAD がマージコミットであること、各ブランチが取り込まれていること）を行う

### Decision: resolve の実行ディレクトリを repo root に固定
- Git 操作は repo root で実行されるべきであるため、resolve_command の起動時に cwd を repo root に設定する
- これにより、プロンプトに `cd ...` を埋め込むことに依存しない

### Decision: マージコミットメッセージ規約
- 逐次マージの各マージコミットメッセージは、対象ブランチに対応する `change_id` を含む
- 例（概念）: `Merge change: <change_id>` のように、機械的に検証できる形式を推奨する

## Prompt Design
resolve_command に渡すプロンプトは、少なくとも以下を含む:
- ターゲットブランチ名（元ブランチ）
- マージ対象ブランチの順序付きリスト
- 各ブランチから抽出した change_id（検証用）
- 期待するコミットメッセージ規約
- pre-commit がファイルを修正してコミットが止まった場合の再試行手順（再ステージ→再コミット）
- 成功条件: ターゲットブランチでマージコミットが作成され、処理対象ブランチが順次取り込まれている

## Risks / Trade-offs
- LLM が Git 操作まで実行するため、誤操作リスクが増える
  - Mitigation: 非破壊的な手順を優先し、`reset --hard` 等を禁止するガードレールをプロンプトに明記する
  - Mitigation: オーケストレータ側で成功条件を機械的に検証する
- pre-commit / hooks の環境差により挙動が変わる
  - Mitigation: フックがファイル修正→コミット中断するパターンを「再ステージ→再コミット」で収束させる方針を明記する

## Migration Plan
- 既存の `resolve_command` 設定は維持しつつ、用途を「競合解消」から「マージ完了」へ拡張する
- 既存テストを更新し、resolve をモックした逐次マージ成功パスを追加する

## Open Questions
- マージコミットのメッセージ形式を固定文字列にするか（検証容易性）、ある程度自由にするか
- 逐次マージの順序規則（現状: workspace 作成順）を厳密に維持するか
