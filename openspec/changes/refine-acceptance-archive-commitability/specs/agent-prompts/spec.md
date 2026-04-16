## MODIFIED Requirements

### Requirement: acceptance プロンプトは差分コンテキストを提示する

acceptance プロンプトは `<acceptance_diff_context>` ブロックで差分レビュー対象を提示しなければならない（MUST）。初回は base branch と現在コミットの差分ファイル一覧を含め、2回目以降は前回 acceptance のコミットからの差分ファイル一覧と前回 findings を含める（MUST）。

acceptance プロンプトは、レビュー対象が archive へ進む前に **final archive commit が実際に成立するか** を確認する指示を含めなければならない（MUST）。ここで確認対象となるのは、archive フローに必要な commit を実際に阻害する blocker のみであり、pre-commit hook、test、lint、format、または特定言語の build/test tool の存在を一般論として仮定してはならない（MUST NOT）。

acceptance は、archive フェーズで初めて発火する commit-path blocker を acceptance で先に露出しなければならない（MUST）。ただし、test / lint / format などを独立の一般 quality gate として追加要求してはならず（MUST NOT）、それらが実際の commit path を阻害する仕組みの一部である場合に限って、commitability の文脈で扱ってよい（MAY）。

Conflux core の acceptance prompt builder は、特定アーキテクチャ・特定言語・特定 repository workflow に依存する gate をハードコードしてはならない（MUST NOT）。固定で埋め込める内容は、`load skills: cflx-*` のような workflow skill 読み込み、change metadata、paths、machine-readable protocol などの最小限に限定されなければならない（MUST）。

#### Scenario: acceptance prompts archive-commitability verification

- **GIVEN** acceptance プロンプトが archive 前の最終レビューとして生成される
- **WHEN** acceptance が実行される
- **THEN** プロンプトは final archive commit の成立を阻害する実 blocker がないか確認するよう指示する
- **AND** その blocker failure を単なる後続 archive 問題として見逃さない

#### Scenario: acceptance does not assume generic test or lint gates

- **GIVEN** target repository に test、lint、format、pre-commit hook の一部または全部が存在しない
- **WHEN** acceptance プロンプトを構築する
- **THEN** プロンプトはそれらの存在を前提にしない
- **AND** archive commit の成立可否のみを readiness の中心として扱う

#### Scenario: commit-path hook remains relevant when it blocks archive commit

- **GIVEN** target repository では通常の commit 実行時に pre-commit hook が走る
- **AND** その hook failure が archive commit を実際に阻害する
- **WHEN** acceptance が archive-readiness を判定する
- **THEN** acceptance はその commit-path blocker を relevant な readiness finding として扱う
- **AND** hook 内部の test/lint/format を独立 gate として追加列挙しない
