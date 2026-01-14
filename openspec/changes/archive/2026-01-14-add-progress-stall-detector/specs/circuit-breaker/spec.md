# Circuit Breaker Capability

## MODIFIED Requirements

### Requirement: 進捗停滞検出（空WIPコミット連続）

Orchestrator は、同一 change に対する `apply` / `archive` の反復で **空の WIP コミット**が連続した場合に停滞（stall）と判定し、その change を停止して無限ループを防止しなければならない（SHALL）。

- 対象 phase: `apply`, `archive`
- 対象モード: serial, parallel
- `resolve` は別サーキットブレーカーで扱う（今回 out of scope）

ここで「空の WIP コミット」とは、`--allow-empty` により作成された WIP コミットのうち、`HEAD` と `HEAD^` の差分がゼロであるコミットを指す。

#### Scenario: Serial apply で空WIPが3回連続したら change を停止し、独立 change を継続
- **GIVEN** ある change `A` の apply 反復ごとに WIP コミットが作成される
- **AND** `A` の直近3つの apply WIP が空コミットである
- **AND** queued な change `B` が存在し、`B` は `A` に依存していない
- **WHEN** orchestrator が次の apply を実行しようとする
- **THEN** stall を検出して `A` を停止（failed 扱い）にする
- **AND** `A` の次の apply は実行しない
- **AND** `B` の実行を開始できる

#### Scenario: Parallel apply で空WIPが3回連続したら change を停止し、依存 change をスキップ
- **GIVEN** parallel 実行で change `A` が apply 反復を繰り返している
- **AND** `A` の直近3つの apply WIP が空コミットである
- **AND** queued な change `C` が存在し、`C` は `A` に依存している
- **AND** queued な change `B` が存在し、`B` は `A` に依存していない
- **WHEN** stall を検出する
- **THEN** `A` を停止（failed 扱い）にする
- **AND** `C` は依存失敗としてスキップされる
- **AND** `B` は実行を継続できる

#### Scenario: Serial archive retry で空WIPが3回連続したら archive を中断して change を停止
- **GIVEN** ある change `A` が archive の検証失敗により retry ループに入っている
- **AND** 各 archive attempt の後に `WIP(archive)` コミットが作成される
- **AND** `A` の直近3つの `WIP(archive)` が空コミットである
- **WHEN** orchestrator が次の archive attempt を実行しようとする
- **THEN** stall を検出して `A` を停止（failed 扱い）にする
- **AND** `A` の archive retry は継続しない

#### Scenario: 設定で threshold を変更できる
- **GIVEN** config 内で `stall_detection.threshold = 2` が設定されている
- **AND** ある change の直近2つの対象 phase の WIP が空コミットである
- **WHEN** 次の反復が開始される
- **THEN** stall が検出される

#### Scenario: 設定で停滞検出を無効化できる
- **GIVEN** config 内で `stall_detection.enabled = false` が設定されている
- **AND** ある change が空WIPを10回連続で作成している
- **WHEN** orchestrator が反復処理を続ける
- **THEN** stall は検出されない
- **AND** 既存のループ制御（例: max_iterations）に従って動作する
