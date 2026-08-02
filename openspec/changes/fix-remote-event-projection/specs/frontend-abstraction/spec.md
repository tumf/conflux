## MODIFIED Requirements

### Requirement: Core / Frontend 状態所有の境界

Core が所有する reducer state と display status はすべてのfrontendに対する正規ソースでなければならない（MUST）。一つのinternal execution eventはCoreで一度だけ適用され、Frontendは受信済みeventや中間frontend modelを使ってCore stateを再適用・再導出してはならない（MUST NOT）。

#### Scenario: One event has one authoritative transition

- **GIVEN** Coreが一つのexecution eventを発行する
- **WHEN** TUIとv2 projectionが同じevent/state outputを受信する
- **THEN** reducer transitionは一度だけ発生する
- **AND** 各frontendは同じauthoritative stateを描画する
- **AND** v2は中間frontend modelからfieldを再導出しない

#### Scenario: Late completion preserves terminal mode

- **GIVEN** CoreがErrorまたはStoppedをauthoritative terminal modeとして保持している
- **WHEN** 遅延または重複したAllCompleted eventが到着する
- **THEN** TUIとv2 projectionは同じmode preservation ruleを適用する
- **AND** frontendごとに異なるterminal stateを表示しない

### Requirement: EventSink トレイトによるフロントエンド抽象化

Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の間に `EventSink` トレイトを定義しなければならない（MUST）。オーケストレーションループは一つのdispatch ownerを通してeventをreducerへ適用し、そのauthoritative event/state outputをfrontend sinkへfan outしなければならない（MUST）。Frontend sinkはreducer stateを直接再適用してはならない（MUST NOT）。

#### Scenario: Structured logs reach each frontend once

- **GIVEN** serialまたはparallel orchestrationが一つのstructured log eventを発行する
- **WHEN** dispatch ownerがfrontend sinkへ配信する
- **THEN** TUIとv2は同じlogを受信する
- **AND** v2 retained logには高々一件だけ保存される
- **AND** log-only eventはworkflow state revisionを進めない

#### Scenario: Duplicate sink delivery is harmless

- **GIVEN** 同じevent identityがfrontend boundaryで重複して観測される
- **WHEN** v2 projectionが処理する
- **THEN** reducer stateは再適用されない
- **AND** event sequence、revision、retained logは重複しない
