## MODIFIED Requirements

### Requirement: Orchestration loop runs apply and archive
`run` サブコマンドは OpenSpec change workflow のオーケストレーションループを実行しなければならない（SHALL）。
オーケストレーターは apply 成功後に acceptance ループを実行し、archive 開始前に結果を判定しなければならない（SHALL）。
acceptance ループは change に対して `acceptance_command` を実行し、出力テキストから pass/fail/continue/blocked を判定して処理を分岐しなければならない（SHALL）。
- exit code はコマンド実行成否のみを示し、acceptance 判定には使用しない。
- acceptance prompt はハードコードされた acceptance prompt の後に設定値の `acceptance_prompt` を連結しなければならない（MUST）。
- acceptance verdict parsing は PASS/FAIL/CONTINUE/BLOCKED マーカーが非意味的な装飾（Markdown 強調など）を伴っていても認識しなければならない（MUST）。
- acceptance verdict parsing は行頭からの前方一致（`starts_with`）を用い、マーカー文字列の後にテキストが続いていても認識しなければならない（MUST）。完全一致を要求してはならない（MUST NOT）。
- acceptance が FAIL の場合、apply ループへ戻る前に tasks.md を更新しなければならない（MUST）。
- tasks.md の更新は、acceptance の失敗回数に対応する `## Acceptance #<n> Failure Follow-up` セクションを末尾に追加するか、既存の関連タスクを未完了に戻す形で行わなければならない（MUST）。
- `Acceptance #<n> Failure Follow-up` の `<n>` は当該 acceptance 試行の 1 始まりの試行番号と一致しなければならない（MUST）。
- acceptance の失敗理由（findings）は tasks.md に記録しなければならない（MUST）。
- 失敗理由の記録は acceptance エージェントが tasks.md を直接編集して行い、オーケストレーターは acceptance 出力から findings を抽出して tasks.md に追記してはならない（MUST NOT）。
- acceptance prompt は FAIL 時に tasks.md の follow-up を更新する指示を含めなければならない（MUST）。
- `Acceptance #<n> Failure Follow-up` セクションでは、各 finding を `- [ ] <finding>` の未完了タスクとして 1 行ずつ記録しなければならない（MUST）。番号付きの箇条書きを使用してはならない（MUST NOT）。
- follow-up セクションに `Address acceptance findings:` のようなラッパー行やネストされた箇条書きを追加してはならない（MUST NOT）。
- acceptance の findings として扱う内容は stdout/stderr の tail 行を用い、`ACCEPTANCE:` マーカーと `FINDINGS:` 行を除外しなければならない（MUST）。
- findings の `- ` 箇条書き構造を解析して抽出してはならない（MUST NOT）。
- ログで件数を表示する場合は findings ではなく tail 行数である旨を明示しなければならない（MUST）。曖昧な "N findings" 表現を使用してはならない（MUST NOT）。
- apply ループは acceptance failure 後も同じ iteration カウンター値で再開しなければならない（MUST）。
- 出力が CONTINUE を示す場合、オーケストレーターは `acceptance_max_continues` 回まで acceptance を再試行しなければならない（MUST）。
- acceptance マーカーが存在しない場合、オーケストレーターは CONTINUE として扱い、`acceptance_max_continues` に従って再試行しなければならない（MUST）。
- CONTINUE の上限を超えた場合、オーケストレーターは FAIL として扱い apply ループへ戻らなければならない（MUST）。
- acceptance 失敗後に apply ループへ戻る際、acceptance ループの iteration カウンターを引き継がなければならない（MUST）。
- acceptance ループの iteration カウンターは試行ごとに増加し、acceptance failure 後に apply ループへ戻ってもリセットしてはならない（MUST NOT）。
- 2 回目以降の acceptance は、前回の acceptance 以降に更新されたファイル一覧と過去の findings に集中し、フルチェックを行ってはならない（MUST NOT）。
- 2 回目以降の acceptance prompt は、前回 acceptance 以降に更新されたファイル一覧（パスのみ）を含めなければならない（MUST）。
- 2 回目以降の acceptance prompt は、前回の acceptance findings を含め、解消確認を指示しなければならない（MUST）。
- 2 回目以降の acceptance prompt は、必要に応じて関連ファイルを読むよう指示し、diff 内容を含めてはならない（MUST NOT）。
- 2 回目以降の acceptance prompt は、前回の acceptance コマンド出力（stdout_tail/stderr_tail）を `<last_acceptance_output>` タグで囲んで含めなければならない（MUST）。
- acceptance コマンド出力は `AcceptanceHistory` に既に保存されているため、新規フィールド追加なしで参照可能でなければならない（MUST）。
- acceptance が BLOCKED の場合、オーケストレーターは当該 change の apply ループを停止し、再試行してはならない（MUST NOT）。
- acceptance が BLOCKED の場合、当該 change は停止状態として記録し、次の change 処理へ進まなければならない（MUST）。
- acceptance 出力ストリーミングループは ACCEPTANCE マーカーを検出した後、設定可能な grace period（デフォルト 30 秒）を開始しなければならない（MUST）。grace period 中はエージェントプロセスの追加出力を引き続き読み取る。grace period が満了してもプロセスが終了していない場合、プロセスを terminate して出力ループを終了しなければならない（MUST）。
- grace period の目的は、エージェントの子プロセス（MCP サーバー等）が stdout/stderr パイプを保持し続けてプロセス終了を妨げる場合の無限ブロックを防ぐことである。

#### Scenario: Acceptance retry narrows to updated files and prior findings
- **GIVEN** change が apply iteration を正常完了する
- **AND** acceptance output が CONTINUE を示す
- **WHEN** オーケストレーターが同じ change に対して次の acceptance を実行する
- **THEN** acceptance prompt は前回の acceptance 以降の更新ファイル一覧のみを含む（diff content なし）
- **AND** acceptance prompt は前回の acceptance findings を含み、解消確認を指示する
- **AND** acceptance prompt は必要に応じて関連ファイルを読むよう指示し、diff 内容を含めない

#### Scenario: Acceptance failure follow-up uses numbered section and flat tasks
- **GIVEN** acceptance output が FAIL で 2 件の findings を含む
- **WHEN** acceptance エージェントが指示に従って tasks.md を更新する
- **THEN** tasks.md の末尾に `## Acceptance #1 Failure Follow-up` が追加される
- **AND** セクション内に `- [ ] <finding>` の未完了タスクが 2 行追加される
- **AND** `Address acceptance findings` のようなラッパー行やネスト箇条書きは含まれない

#### Scenario: CONTINUE tail propagation to next acceptance prompt
- **GIVEN** acceptance output が CONTINUE を示す
- **AND** `AcceptanceHistory` に前回の acceptance 試行が記録されている
- **WHEN** オーケストレーターが acceptance ループを継続する
- **THEN** 次の acceptance プロンプトに `<last_acceptance_output>` タグで囲まれた前回の stdout_tail/stderr_tail が含まれる
- **AND** エージェントは前回の調査結果を参照して次の調査アクションを決定できる

#### Scenario: Acceptance failure logging avoids misleading findings count
- **GIVEN** acceptance output の tail に `ACCEPTANCE: FAIL` と `FINDINGS:` が含まれる
- **WHEN** オーケストレーターが acceptance FAIL を記録する
- **THEN** findings として保存される tail 行から `ACCEPTANCE:` マーカーと `FINDINGS:` 行が除外される
- **AND** ログは "N findings" のような誤解を招く件数表現を出さない

#### Scenario: Acceptance blocked stops apply loop
- **GIVEN** acceptance output が `ACCEPTANCE: BLOCKED` を示す
- **WHEN** オーケストレーターが acceptance 結果を処理する
- **THEN** 当該 change の apply ループは停止する
- **AND** 同一 change の apply は再試行されない

#### Scenario: Verdict with trailing text is recognized
- **GIVEN** acceptance エージェントが `ACCEPTANCE: PASSAll acceptance criteria verified:` のように verdict マーカーの後に改行なしでテキストを続けて出力する
- **WHEN** オーケストレーターが acceptance 出力をパースする
- **THEN** verdict は PASS として正しく認識される
- **AND** 後続テキストの有無にかかわらず判定が変わらない

#### Scenario: Grace period terminates stale agent process
- **GIVEN** acceptance エージェントが `ACCEPTANCE: PASS` を出力する
- **AND** エージェントプロセスが MCP サーバー等の子プロセスにより stdout パイプが閉じられず終了しない
- **WHEN** grace period（デフォルト 30 秒）が満了する
- **THEN** オーケストレーターはエージェントプロセスを terminate する
- **AND** 収集済みの出力に基づいて acceptance 結果を判定し次のステップに進む
