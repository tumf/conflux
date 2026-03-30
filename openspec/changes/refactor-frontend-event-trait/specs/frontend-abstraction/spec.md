## ADDED Requirements

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
