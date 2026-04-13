## MODIFIED Requirements

### Requirement: EventSink トレイトによるフロントエンド抽象化

Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の間に `EventSink` トレイトを定義しなければならない（MUST）。

オーケストレーションループはフロントエンド固有の型（`mpsc::Sender<OrchestratorEvent>`, `WebState`）に直接依存してはならない（SHALL NOT）。代わりに `EventSink` トレイトを通じてイベントを配信する。

Frontend は Core に対して `EventSink` 経由でイベントを受信し、`ReducerCommand` 経由でコマンドを発行しなければならない（MUST）。この 2 つが Core / Frontend 間の唯一の通信経路である。

本 Requirement は旧 spec 内で 2 回重複していた同名 Requirement を 1 つに統合したものである（旧版には `ReducerCommand` による Frontend → Core 通信経路の規定が欠落していた）。

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
- **THEN** Frontend は `apply_command()` を通じて `ReducerCommand` を発行する
- **AND** Core の状態を直接変更しない
