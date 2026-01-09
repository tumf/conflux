# cli Specification Delta

## MODIFIED Requirements

### Requirement: TUIレイアウト構成

TUIは選択モードと実行モードで適切なレイアウトを表示しなければならない（SHALL）。

#### Scenario: 選択モードのレイアウト
- **WHEN** TUIが選択モードである
- **THEN** ヘッダー（タイトル、モード表示、自動更新インジケーター）が上部に表示される
- **AND** 操作ヘルプ（↑↓: move, Space: toggle, F5: run, q: quit）が表示される
- **AND** チェックボックス付き変更リストが中央に表示される
- **AND** 選択件数・新規件数がフッターに表示される
- **AND** アプリケーション状態に応じたガイダンスメッセージがフッターに表示される

#### Scenario: 実行モードのレイアウト
- **WHEN** TUIが実行モードである
- **THEN** ヘッダー（タイトル、Running/Completedステータス、自動更新インジケーター）が上部に表示される
- **AND** キュー状態付き変更リストが表示される
- **AND** 現在処理パネル（変更ID、ステータス）が表示される
- **AND** ログパネルが下部に表示される

## ADDED Requirements

### Requirement: フッターの動的ガイダンス表示

選択モードのフッターは、アプリケーションの状態に応じて適切なガイダンスメッセージを表示しなければならない（SHALL）。

#### Scenario: 変更がない場合のガイダンス
- **WHEN** TUIが選択モードである
- **AND** 変更リストが空である
- **THEN** フッターに "Add new proposals to get started" と表示される

#### Scenario: 変更が未選択の場合のガイダンス
- **WHEN** TUIが選択モードである
- **AND** 変更が1つ以上存在する
- **AND** 選択されている変更が0件である
- **THEN** フッターに "Select changes with Space to process" と表示される

#### Scenario: 変更が選択済みの場合のガイダンス
- **WHEN** TUIが選択モードである
- **AND** 1つ以上の変更が選択されている
- **THEN** フッターに "Press F5 to start processing" と表示される

### Requirement: 実行中フッターの進捗バー表示

実行モードのフッターには、全体の処理進捗をバーで表示しなければならない（SHALL）。

#### Scenario: 実行中の進捗バー表示
- **WHEN** TUIが実行モードである
- **THEN** フッターにキュー内全タスクの進捗バーが表示される
- **AND** 進捗バーは完了タスク数/総タスク数に基づいて計算される
- **AND** パーセンテージが数値で表示される

#### Scenario: 進捗バーの計算方法
- **WHEN** 進捗バーを表示する
- **THEN** 総タスク数は選択された全変更の `total_tasks` の合計である
- **AND** 完了タスク数は選択された全変更の `completed_tasks` の合計である
- **AND** 進捗率は `completed_tasks / total_tasks * 100` で計算される

#### Scenario: タスク数が0の場合
- **WHEN** 進捗バーを表示する
- **AND** 総タスク数が0である
- **THEN** 進捗バーは0%として表示される
