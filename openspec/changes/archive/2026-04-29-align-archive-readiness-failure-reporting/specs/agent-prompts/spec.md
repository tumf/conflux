## MODIFIED Requirements

### Requirement: acceptance プロンプトは差分コンテキストを提示する

acceptance プロンプトは `<acceptance_diff_context>` ブロックで差分レビュー対象を提示しなければならない（MUST）。初回は base branch と現在コミットの差分ファイル一覧を含め、2回目以降は前回 acceptance のコミットからの差分ファイル一覧と前回 findings を含める（MUST）。

acceptance プロンプトは、レビュー対象が archive へ進む前に **final archive commit が実際に成立するか** を確認する指示を含めなければならない（MUST）。ここで確認対象となるのは、archive フローに必要な commit を実際に阻害する blocker のみであり、pre-commit hook、test、lint、format、または特定言語の build/test tool の存在を一般論として仮定してはならない（MUST NOT）。

acceptance は、archive フェーズで初めて発火する commit-path blocker を acceptance で先に露出しなければならない（MUST）。ただし、test / lint / format などを独立の一般 quality gate として追加要求してはならず（MUST NOT）、それらが実際の commit path を阻害する仕組みの一部である場合に限って、commitability の文脈で扱ってよい（MAY）。

Conflux core の acceptance prompt builder は、特定アーキテクチャ・特定言語・特定 repository workflow に依存する gate をハードコードしてはならない（MUST NOT）。固定で埋め込める内容は、`load skills: cflx-*` のような workflow skill 読み込み、change metadata、paths、machine-readable protocol などの最小限に限定されなければならない（MUST）。

archive readiness blocker が acceptance または archive CLI の事前検証で明示された場合、その blocker は downstream の archive 失敗表示でも primary root cause として保持されなければならない（MUST）。後続の file-state verification failure は補助説明として追加してよいが、earlier blocker summary を消してはならない（MUST NOT）。

#### Scenario: acceptance-detected archive blocker survives later archive verification noise

- **GIVEN** acceptance が change `beta` に対して archive readiness blocker を finding として記録している
- **AND** その後の archive attempt では `openspec/changes/beta` が残って file-state verification も失敗する
- **WHEN** orchestrator が最終 archive failure を履歴またはユーザー向けに整形する
- **THEN** final message には acceptance/validation で見つかった blocker summary が含まれる
- **AND** `changes` ディレクトリ残留は補助文脈としてのみ追加される
