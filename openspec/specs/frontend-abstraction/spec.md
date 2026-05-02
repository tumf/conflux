## Purpose

Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の間の抽象化レイヤーを定義し、疎結合なアーキテクチャを維持する。

## Requirements

### Requirement: Core / Frontend 状態所有の境界

Core が所有する display status の正規ソースは、dependency wait の `blocked`（canonical concept: `dependency-blocked`）と、apply/rejecting/acceptance hold を含む `stalled` を区別しなければならない（MUST）。

acceptance gate observation は `acceptance-gated` メタデータとして保持してよいが、display status として `gated` を露出してはならない（MUST NOT）。

Frontend はこれらを独自の lifecycle copy や render-time simplification によって単一の `blocked` へ collapse してはならない（MUST NOT）。

#### Scenario: Frontend keeps blocked and stalled distinct
- **GIVEN** Core が blocker-adjacent display status を提供している
- **WHEN** TUI または Web UI が change row / API payload / status badge を描画する
- **THEN** dependency wait は `blocked` として表示される
- **AND** apply-side または acceptance-side の resumable hold は `stalled` として表示される
- **AND** Frontend はそれらを単一の `blocked` 値へ変換しない

### Requirement: EventSink トレイトによるフロントエンド抽象化

Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の間に `EventSink` トレイトを定義しなければならない（MUST）。

オーケストレーションループはフロントエンド固有の型（`mpsc::Sender<OrchestratorEvent>`, `WebState`）に直接依存してはならない（SHALL NOT）。代わりに `EventSink` トレイトを通じてイベントを配信する。

Frontend は Core に対して `EventSink` 経由でイベントを受信し、`ReducerCommand` 経由でコマンドを発行しなければならない（MUST）。この 2 つが Core / Frontend 間の唯一の通信経路である。

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
