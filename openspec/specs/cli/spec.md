# cli Specification

## Purpose
TBD - created by archiving change add-run-subcommand. Update Purpose after archive.
## Requirements
### Requirement: サブコマンド構造

CLI はサブコマンド構造を持ち、将来的なコマンド拡張に対応できなければならない（SHALL）。

#### Scenario: サブコマンドなしで実行
- **WHEN** ユーザーが引数なしで `openspec-orchestrator` を実行する
- **THEN** 利用可能なサブコマンド一覧を含むヘルプメッセージを表示する

#### Scenario: 不明なサブコマンドで実行
- **WHEN** ユーザーが存在しないサブコマンドで実行する
- **THEN** エラーメッセージと利用可能なサブコマンド一覧を表示する

### Requirement: run サブコマンド

`run` サブコマンドは OpenSpec 変更ワークフローのオーケストレーションループを実行しなければならない（SHALL）。

#### Scenario: run サブコマンドの基本実行
- **WHEN** ユーザーが `openspec-orchestrator run` を実行する
- **THEN** オーケストレーションループが開始される

#### Scenario: 特定の変更を指定して実行
- **WHEN** ユーザーが `openspec-orchestrator run --change <id>` を実行する
- **THEN** 指定された変更のみを処理する

#### Scenario: opencode パスのカスタマイズ
- **WHEN** ユーザーが `openspec-orchestrator run --opencode-path <path>` を実行する
- **THEN** 指定されたパスの opencode バイナリを使用する

#### Scenario: openspec コマンドのカスタマイズ
- **WHEN** ユーザーが `openspec-orchestrator run --openspec-cmd <cmd>` を実行する
- **THEN** 指定されたコマンドで openspec を実行する

### Requirement: デフォルトTUI起動

サブコマンドなしで起動した場合、インタラクティブTUIを表示しなければならない（SHALL）。

#### Scenario: サブコマンドなしでの起動
- **WHEN** ユーザーが `openspec-orchestrator` を引数なしで実行する
- **THEN** インタラクティブTUIが起動する
- **AND** 選択モードで変更一覧が表示される

#### Scenario: runサブコマンドでの起動（後方互換性）
- **WHEN** ユーザーが `openspec-orchestrator run` を実行する
- **THEN** 従来通りオーケストレーションループが直接実行される

### Requirement: 変更選択モード

TUI起動時、変更選択モードを表示し、ユーザーが処理する変更を選択できなければならない（SHALL）。

#### Scenario: 変更一覧の初期表示
- **WHEN** TUIが選択モードで起動する
- **THEN** 全ての既存変更がチェックボックス付きリストで表示される
- **AND** 既存の変更はデフォルトで選択状態である
- **AND** カーソルが最初の変更に位置する

#### Scenario: カーソル移動
- **WHEN** ユーザーが↑キーまたは↓キーを押す
- **THEN** カーソルが上下に移動する
- **AND** 現在のカーソル位置が視覚的に示される（`►` マーカー）

#### Scenario: 選択トグル
- **WHEN** ユーザーがSpaceキーを押す
- **THEN** カーソル位置の変更の選択状態が切り替わる
- **AND** チェックボックスの表示が更新される（`[x]` ↔ `[ ]`）
- **AND** 選択件数の表示が更新される

#### Scenario: 終了
- **WHEN** ユーザーが `q` キーを押す
- **THEN** TUIが終了し、ターミナルが元の状態に復元される

### Requirement: 選択変更の実行開始

選択モードでF5キーを押すと、選択された変更の処理を開始しなければならない（SHALL）。

#### Scenario: F5キーで実行開始
- **WHEN** ユーザーがF5キーを押す
- **AND** 1つ以上の変更が選択されている
- **THEN** TUIが実行モードに切り替わる
- **AND** 選択された変更がキューに追加される

#### Scenario: 選択なしでF5キー
- **WHEN** ユーザーがF5キーを押す
- **AND** 変更が1つも選択されていない
- **THEN** 実行は開始されない
- **AND** 警告メッセージが表示される

### Requirement: 実行モードダッシュボード

実行モードでは、処理中の変更の進捗状況をダッシュボード形式で表示しなければならない（SHALL）。

#### Scenario: 変更一覧の進捗表示

- **WHEN** TUIが実行モードである
- **THEN** 全ての変更がキュー状態と共に表示される
- **AND** 各変更の完了タスク数/総タスク数とパーセンテージが表示される

#### Scenario: キュー状態の表示

- **WHEN** TUIが実行モードである
- **THEN** 処理中の変更は進捗バーと共に表示される
- **AND** キュー待機中の変更は「queued」と表示される
- **AND** 未選択の変更は「not queued」と表示される
- **AND** エラーが発生した変更は「error」と赤色で表示される

#### Scenario: 現在処理中の変更のハイライト

- **WHEN** 変更が処理中である
- **THEN** 処理中の変更が視覚的にハイライトされる（`►` マーカー）
- **AND** ステータスパネルに変更IDと処理状況が表示される

#### Scenario: ログのリアルタイム表示

- **WHEN** オーケストレーションが実行中である
- **THEN** ログメッセージがログパネルにリアルタイムで追加される
- **AND** 最新のログが常に表示される（自動スクロール）

#### Scenario: 処理完了時の表示

- **WHEN** 全てのキュー内変更の処理が完了する
- **THEN** ヘッダーのステータスが「Completed」に変更される
- **AND** TUIは表示を維持し、ユーザーが `q` キーで終了できる

#### Scenario: エラー発生時の表示

- **WHEN** 変更の処理中にエラーが発生する
- **THEN** ヘッダーのステータスが「Error」と赤色で表示される
- **AND** ステータスパネルにエラー情報と「Press F5 to retry」が表示される
- **AND** TUIは表示を維持し、ユーザーがF5でリトライまたは`q`キーで終了できる

### Requirement: TUIレイアウト構成

TUIは選択モードと実行モードで適切なレイアウトを表示しなければならない（SHALL）。

#### Scenario: 選択モードのレイアウト
- **WHEN** TUIが選択モードである
- **THEN** ヘッダー（タイトル、モード表示、自動更新インジケーター）が上部に表示される
- **AND** 操作ヘルプ（↑↓: move, Space: toggle, F5: run, q: quit）が表示される
- **AND** チェックボックス付き変更リストが中央に表示される
- **AND** 選択件数・新規件数とF5実行の案内がフッターに表示される

#### Scenario: 実行モードのレイアウト
- **WHEN** TUIが実行モードである
- **THEN** ヘッダー（タイトル、Running/Completedステータス、自動更新インジケーター）が上部に表示される
- **AND** キュー状態付き変更リストが表示される
- **AND** 現在処理パネル（変更ID、ステータス）が表示される
- **AND** ログパネルが下部に表示される

#### Scenario: 最小ターミナルサイズ
- **WHEN** ターミナルサイズが80x24未満である
- **THEN** 警告メッセージを表示するか、レイアウトを簡略化する

### Requirement: 自動更新機能

TUIは定期的に変更一覧を自動更新しなければならない（SHALL）。

#### Scenario: 定期的な自動更新
- **WHEN** TUIが表示されている
- **THEN** 5秒間隔で `openspec list` が実行される
- **AND** 変更一覧の進捗状況が更新される

#### Scenario: 自動更新インジケーター
- **WHEN** TUIが表示されている
- **THEN** ヘッダーに自動更新間隔とインジケーター（`Auto-refresh: 5s ↻`）が表示される

#### Scenario: 更新中の表示継続
- **WHEN** 自動更新が実行中である
- **THEN** TUIの表示は中断されない
- **AND** 更新完了後に変更一覧が反映される

### Requirement: 新規変更検出

自動更新時に新しい変更が検出された場合、適切に表示しなければならない（SHALL）。

#### Scenario: 新規変更の検出
- **WHEN** 自動更新により新しい変更が検出される
- **THEN** 新しい変更が変更一覧に追加される
- **AND** 「NEW」バッジが表示される
- **AND** ログに「Discovered new change: <id>」と出力される

#### Scenario: 新規変更のデフォルト状態
- **WHEN** 新しい変更が検出される
- **THEN** その変更はデフォルトで未選択状態（`[ ]`）である
- **AND** フッターの新規件数が更新される

#### Scenario: NEWバッジの表示
- **WHEN** 変更が新規検出されたものである
- **THEN** 変更名の横に「NEW」バッジが表示される
- **AND** バッジは視覚的に目立つ色で表示される

### Requirement: 動的実行キュー

実行モードで未選択の変更を選択するとキューに追加でき、キュー待機中の変更を解除できなければならない（SHALL）。

#### Scenario: 実行中のキュー追加

- **WHEN** TUIが実行モードである
- **AND** ユーザーが未選択の変更（NotQueued）にカーソルを合わせてSpaceキーを押す
- **THEN** その変更が実行キューに追加される
- **AND** 表示が「not queued」から「queued」に更新される

#### Scenario: キュー待機中の変更を解除

- **WHEN** TUIが実行モードである
- **AND** ユーザーがキュー待機中（Queued）の変更にカーソルを合わせてSpaceキーを押す
- **THEN** その変更がキューから取り除かれる
- **AND** 表示が「queued」から「not queued」に更新される
- **AND** 選択状態が解除される

#### Scenario: キュー追加後の処理順序

- **WHEN** 変更が動的にキューに追加される
- **THEN** その変更は現在処理中の変更の完了後に処理される
- **AND** 既にキュー内にある変更の順序は変わらない

#### Scenario: 処理中の変更は変更不可

- **WHEN** 変更が処理中（Processing）である
- **THEN** その変更の選択状態は変更できない
- **AND** Spaceキーを押しても何も起こらない

#### Scenario: アーカイブ中の変更は変更不可

- **WHEN** 変更がアーカイブ処理中である
- **THEN** その変更の選択状態は変更できない
- **AND** Spaceキーを押しても何も起こらない

### Requirement: エラー状態の表示

エラー発生時、TUIはエラー状態を明示的に表示しなければならない（SHALL）。

#### Scenario: エラー発生時のモード遷移

- **WHEN** opencode実行がエラー（LLMエラー、料金不足等）で失敗する
- **THEN** TUIのモードが「Error」に遷移する
- **AND** ヘッダーのステータスが「Error」と赤色で表示される

#### Scenario: ステータスパネルのエラー表示

- **WHEN** TUIがエラー状態である
- **THEN** ステータスパネルに「Error in <change_id>」と表示される
- **AND** 「Press F5 to retry」のガイダンスが表示される

#### Scenario: エラー状態でのChange表示

- **WHEN** TUIがエラー状態である
- **THEN** エラーが発生したChangeのステータスは「[error]」と赤色で表示される
- **AND** 他のqueued状態のChangeはそのまま維持される

### Requirement: F5キーでのエラーリトライ

エラー状態でF5キーを押すと、エラーが発生したChangeの処理をリトライできなければならない（SHALL）。

#### Scenario: F5キーでリトライ開始

- **WHEN** TUIがエラー状態である
- **AND** ユーザーがF5キーを押す
- **THEN** エラー状態のChangeが再度キューに追加される
- **AND** TUIが「Running」モードに遷移する
- **AND** 処理が再開される

#### Scenario: リトライ時のログ表示

- **WHEN** ユーザーがF5キーでリトライを開始する
- **THEN** ログパネルに「Retrying: <change_id>」と表示される

#### Scenario: リトライ成功後の状態

- **WHEN** リトライした処理が成功する
- **THEN** Changeのステータスが「completed」または「archived」に更新される
- **AND** 残りのキュー内Changeがあれば続けて処理される

