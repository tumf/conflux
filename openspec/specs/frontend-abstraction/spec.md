## Requirements

### Requirement: EventSink トレイトによるフロントエンド抽象化
Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の間に `EventSink` トレイトを定義しなければならない（MUST）。

オーケストレーションループはフロントエンド固有の型（`mpsc::Sender<OrchestratorEvent>`, `WebState`）に直接依存してはならない（SHALL NOT）。代わりに `EventSink` トレイトを通じてイベントを配信する。

#### Scenario: TUI がイベントを EventSink 経由で受信する
- **WHEN** オーケストレーションループがイベントを発行する
- **THEN** `TuiEventSink` の `on_event()` が呼ばれる
- **AND** 内部で TUI channel にイベントが転送される

#### Scenario: テスト時にフロントエンドをモックできる
- **WHEN** オーケストレーションのテストでフロントエンドが不要な場合
- **THEN** `MockEventSink` を注入してイベントを収集できる
- **AND** TUI/Web の実体に依存しない

## Requirements

### Requirement: Core / Frontend 状態所有の境界

Conflux のアーキテクチャは **Core**（Reducer + オーケストレーションループ）と **Frontend**（TUI, Web UI）の2層に分離される。各層が所有する状態の範囲を以下のように定義する。

**Core が所有する状態（Frontend は独立コピーを持ってはならない）:**
- Change lifecycle（ActivityState: Applying, Accepting, Archiving, Resolving, Idle）
- Resolve queue（FIFO キュー）と resolve serialization フラグ
- Execution state（apply count, iteration, pending/archived/completed sets）
- Wait state（MergeWait, ResolveWait, DependencyBlocked）
- Terminal state（Archived, Merged, Error）
- Display status の正規ソース（`ChangeRuntimeState::display_status()` から導出）

**Frontend が所有してよい状態（UI 固有状態）:**
- Cursor position, focus, panel selection
- View mode (Changes / Worktrees)
- Selection state (checkboxes, execution marks)
- Sort / filter preferences
- Popup / modal state
- Render cache（display_status_cache, display_color_cache）— Core の正規値から派生し、Core を上書きしない
- Transport / session state（WebSocket 接続状態等）

**Frontend が持ってはならない状態:**
- Change lifecycle の独立コピー（旧 QueueStatus enum 等）
- Resolve queue の独立コピー
- Resolve serialization の判断ロジック（Core の `is_resolving_active()` を参照するのみ）
- Merge / resolve の可否判断に使う独自フラグ（apply/accept/archive をブロックする用途）

Frontend は Core の状態を **読み取り** と **コマンド発行** でのみ操作する。状態遷移は Core の reducer を経由しなければならない（MUST）。

#### Scenario: Frontend は Core lifecycle の独立コピーを持たない

- **GIVEN** TUI または Web UI が Change のステータスを表示する
- **WHEN** ステータスの取得が必要になる
- **THEN** shared orchestration state（OrchestratorState）の `display_status()` から導出された値を使用する
- **AND** Frontend 固有の lifecycle enum や状態マシンを持たない

#### Scenario: Frontend の render cache は Core を上書きしない

- **GIVEN** TUI が display_status_cache を保持している
- **WHEN** Core の display_status が更新される
- **THEN** render cache は Core の最新値で上書きされる
- **AND** render cache から Core への逆方向の上書きは発生しない

#### Scenario: Resolve serialization は Core で判断される

- **GIVEN** ユーザーが resolve 操作を要求する
- **WHEN** Frontend が resolve 可否を判断する必要がある
- **THEN** Core の `is_resolving_active()` を参照する
- **AND** Frontend 独自の resolve serialization フラグでは判断しない
- **AND** この判断は resolve 操作のみに影響し、apply/accept/archive はブロックしない


### Requirement: EventSink トレイトによるフロントエンド抽象化

Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の間に `EventSink` トレイトを定義しなければならない（MUST）。

オーケストレーションループはフロントエンド固有の型（`mpsc::Sender<OrchestratorEvent>`, `WebState`）に直接依存してはならない（SHALL NOT）。代わりに `EventSink` トレイトを通じてイベントを配信する。

Frontend は Core に対して `EventSink` 経由でイベントを受信し、`ReducerCommand` 経由でコマンドを発行する。この2つが Core / Frontend 間の唯一の通信経路である。

#### Scenario: TUI がイベントを EventSink 経由で受信する
- **WHEN** オーケストレーションループがイベントを発行する
- **THEN** `TuiEventSink` の `on_event()` が呼ばれる
- **AND** 内部で TUI channel にイベントが転送される

#### Scenario: テスト時にフロントエンドをモックできる
- **WHEN** オーケストレーションのテストでフロントエンドが不要な場合
- **THEN** `MockEventSink` を注入してイベントを収集できる
- **AND** TUI/Web の実体に依存しない

#### Scenario: Frontend は ReducerCommand 経由でのみ状態を変更する
- **GIVEN** ユーザーが TUI または Web UI で操作を行う
- **WHEN** その操作が Core の状態変更を必要とする
- **THEN** Frontend は `apply_command()` を通じて ReducerCommand を発行する
- **AND** Core の状態を直接変更しない
