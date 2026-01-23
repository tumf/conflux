# Change: acceptance条件にgit clean checkを追加

## Why
acceptance の判定が作業ツリーの汚れを見逃すと、未コミット変更や未追跡ファイルが残ったまま archive に進み、検証結果の再現性やレビューの信頼性が下がるためです。

## What Changes
- acceptance プロンプトに git working tree のクリーン確認を追加する
- dirty な場合は FAIL とし、FINDINGS に未コミット変更と未追跡ファイルを明示する

## Impact
- Affected specs: agent-prompts
- Affected code: src/config/defaults.rs (ACCEPTANCE_SYSTEM_PROMPT)
