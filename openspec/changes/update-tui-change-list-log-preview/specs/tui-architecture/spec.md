## MODIFIED Requirements
### Requirement: Log Entry Structure and Display

TUIログエントリは timestamp、message、color、および任意のコンテキスト情報（change ID、operation、iteration number）を含まなければならない (MUST)。
ログヘッダは利用可能なコンテキスト情報に応じて段階的に表示される。
apply/archive/acceptance/resolve の開始時には、対応する subcommand 文字列が TUI ログに表示されなければならない。
subcommand の出力ログは対応する operation を付与して記録されなければならない。

- operation を持つログ（apply/archive/acceptance/resolve）は、iteration がある場合に `[operation:{iteration}]`、iteration がない場合に `[operation]` 形式でヘッダを表示しなければならない。ヘッダには change_id を表示してはならない。
- change_id を持たない analysis のログ出力は必ず iteration number を含み、ヘッダは `[analysis:{iteration}]` 形式で表示されなければならない。
- auto-scroll が無効な場合、TUI はユーザーが閲覧しているログ範囲を維持し、表示行は新しいログ追加やログバッファのトリミングで移動してはならない。表示行がトリミングされた場合は、最も古い残存ログ行にクランプされなければならず、auto-scroll は自動的に再有効化されてはならない。

#### Scenario: apply/archive/acceptance/resolve の command が表示される
- **GIVEN** change_id が設定され、apply/archive/acceptance/resolve の開始イベントに command が含まれている
- **WHEN** TUI が開始イベントを処理する
- **THEN** ログに `Command:` 行が追加される
- **AND** ログは対応する operation 付きで記録される

#### Scenario: Archive ログは常に iteration 付きで表示される
- **GIVEN** `change_id="test-change"`、`operation="archive"`、`iteration=2` のログエントリが作成される
- **WHEN** TUI がログを描画する
- **THEN** ログヘッダは `[archive:2]` として表示される
- **AND** retry の順序が判別できる

#### Scenario: Analysis ログは iteration 付きで表示される
- **GIVEN** `change_id=None`、`operation="analysis"`、`iteration=3` のログエントリが作成される
- **WHEN** TUI がログを描画する
- **THEN** ログヘッダは `[analysis:3]` として表示される
- **AND** analysis の再実行が区別できる

#### Scenario: auto-scroll が無効なとき表示範囲が固定される
- **GIVEN** ユーザーがログをスクロール済みで auto-scroll が無効になっている
- **WHEN** 新しいログが追加される（必要に応じて古いログがトリミングされる）
- **THEN** 表示範囲は同じログ行を指し続ける
- **AND** 表示範囲がトリミングされた場合、最も古い残存ログ行にクランプされる
- **AND** auto-scroll は自動的に再有効化されない

## ADDED Requirements
### Requirement: Change List Log Preview

TUI の変更一覧は、各 change の最新ログエントリを右側の空きスペースに単一行のプレビューとして表示しなければならない (MUST)。プレビューにはログの相対時刻（1分未満は `just now`、1分以上は `<n><unit> ago` 形式。例: `2m ago`, `3h ago`。相対時刻の値は切り捨てで丸める）と短縮ヘッダ形式 `[operation:{iteration}]` または `[operation]`、およびメッセージが含まれ、表示幅に収まるように折り返しなしで省略されなければならない。

#### Scenario: 変更一覧に最新ログの相対時刻付きプレビューが表示される
- **GIVEN** ある change に 2分前のログエントリ（`operation="resolve"`、`iteration=1`）が存在する
- **WHEN** TUI が変更一覧を描画する
- **THEN** change 行に `2m ago [resolve:1]` と最新ログメッセージが同じ行で表示される
