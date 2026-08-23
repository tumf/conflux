# command-queue Specification

## Purpose
TBD - created by archiving change add-command-execution-queue. Update Purpose after archive.
## Requirements

### Requirement: 時間差起動（Staggered Start）

コマンドキューは連続するコマンド実行の間に設定可能な遅延を導入しなければならない (MUST)。

遅延の動作は以下の通りとする：
- 前回のコマンド実行時刻を記録
- 次のコマンド実行前に、前回実行からの経過時間をチェック
- 設定された遅延時間未満の場合は、残り時間だけ待機
- 遅延時間が経過している場合は即座に実行

#### Scenario: 連続実行時の遅延適用

- **GIVEN** 遅延時間が2秒に設定されている
- **WHEN** コマンドAを実行
- **AND** 0.5秒後にコマンドBを実行しようとする
- **THEN** コマンドBは1.5秒待機してから実行される

#### Scenario: 遅延時間経過後の即座実行

- **GIVEN** 遅延時間が2秒に設定されている
- **WHEN** コマンドAを実行
- **AND** 3秒後にコマンドBを実行しようとする
- **THEN** コマンドBは待機なしで即座に実行される

#### Scenario: 初回実行時の遅延なし

- **GIVEN** コマンドキューが初期化されたばかり
- **WHEN** 最初のコマンドを実行
- **THEN** 遅延なしで即座に実行される（前回実行がないため）

### Requirement: 自動リトライ機構

コマンドキューは一時的なエラーパターンを検出し、自動的にコマンドを再実行しなければならない (MUST)。

リトライの動作は以下の通りとする：
- コマンド実行の開始時刻と終了時刻を記録し、実行時間を計測
- コマンド実行結果（終了ステータスと標準エラー出力）を取得
- 成功（終了ステータス0）の場合は完了
- 失敗の場合、以下の3つの条件でリトライ判定：
  1. 標準エラー出力が設定されたエラーパターンにマッチ
  2. 実行時間が設定された閾値未満（デフォルト: 5秒）
  3. **コマンドが異常終了（exit code != 0）**
- いずれかの条件を満たす場合、リトライ可能エラーと判定（OR条件）
- 最大リトライ回数以内の場合、待機後に再実行
- 最大リトライ回数を超えた場合、エラーを返却

ストリーミング/非ストリーミングの実行経路は、同一のリトライ判定ロジックを共有しなければならない (MUST)。

#### Scenario: エージェントクラッシュでの自動再実行

- **GIVEN** 最大リトライ回数が2回に設定されている
- **WHEN** コマンド実行がエージェントクラッシュ（exit code = 1）で失敗
- **AND** エラーメッセージが設定パターンにマッチしない
- **AND** 実行時間が閾値（5秒）を超えている
- **THEN** exit code != 0 のため、リトライ可能と判定される
- **AND** 設定された待機時間後に自動的に再実行される

### Requirement: 実行時間による一時的エラー判定

コマンドキューは実行時間が極端に短い失敗を一時的エラーと判定しなければならない (MUST)。

判定基準は以下の通りとする：
- コマンド実行の開始から終了までの時間を計測
- 設定された閾値（デフォルト: 5秒）未満で失敗した場合、一時的エラーの可能性ありと判定
- この判定はエラーパターンマッチングと独立して機能（OR条件）
- 実行時間が閾値以上の場合、エラーパターンマッチのみで判定

理論的根拠：
- 起動直後（数秒以内）のエラーは環境問題（モジュール未解決、ファイルロック競合）の可能性が高い
- 長時間実行後のエラーは論理エラーやテスト失敗の可能性が高く、リトライしても無駄

#### Scenario: 起動直後のエラーで自動リトライ

- **GIVEN** リトライ閾値が5秒に設定されている
- **WHEN** コマンド実行が0.5秒で失敗
- **AND** エラーメッセージがリトライパターンにマッチしない
- **THEN** 実行時間が5秒未満のため、リトライ可能と判定される
- **AND** 自動的に再実行される

#### Scenario: 長時間実行後のエラーはリトライしない

- **GIVEN** リトライ閾値が5秒に設定されている
- **WHEN** コマンド実行が120秒で失敗
- **AND** エラーメッセージがリトライパターンにマッチしない
- **THEN** 実行時間が5秒を超えているため、リトライ不可と判定される
- **AND** エラーが返却される（リトライしない）

#### Scenario: エラーパターンマッチとOR条件で判定

- **GIVEN** リトライ閾値が5秒、パターンに `Cannot find module` が設定されている
- **WHEN** コマンド実行が30秒で失敗
- **AND** エラーメッセージが `Cannot find module` を含む
- **THEN** 実行時間は閾値超過だが、パターンマッチのため、リトライ可能と判定される

#### Scenario: 両方の条件を満たさない場合

- **GIVEN** リトライ閾値が5秒、パターンに `Cannot find module` が設定されている
- **WHEN** コマンド実行が30秒で失敗
- **AND** エラーメッセージが `Syntax error` である
- **THEN** 実行時間も閾値超過、パターンも不一致のため、リトライ不可
- **AND** エラーが返却される

### Requirement: エラーパターンマッチング

コマンドキューは正規表現を使用してエラーメッセージをパターンマッチングしなければならない (MUST)。

パターンマッチングの動作は以下の通りとする：
- 設定されたパターンリストを順に評価
- 各パターンを正規表現としてコンパイル
- 標準エラー出力に対して正規表現マッチを実行
- いずれかのパターンにマッチした場合、リトライ可能と判定

#### Scenario: 部分マッチでのパターン検出

- **GIVEN** リトライパターンに `Cannot find module` が設定されている
- **WHEN** 標準エラー出力が `Error: Cannot find module './lib/utils.js' from '/path/to/file'` を含む
- **THEN** パターンにマッチし、リトライ可能と判定される

#### Scenario: 正規表現メタ文字の使用

- **GIVEN** リトライパターンに `EBADF.*lock` が設定されている
- **WHEN** 標準エラー出力が `Error: EBADF: bad file descriptor, realpath 'file.lock'` を含む
- **THEN** 正規表現にマッチし、リトライ可能と判定される

#### Scenario: 複数パターンの評価

- **GIVEN** リトライパターンに `["Cannot find module", "ENOTFOUND"]` が設定されている
- **WHEN** 標準エラー出力が `Error: ENOTFOUND registry.npmjs.org` を含む
- **THEN** 2番目のパターンにマッチし、リトライ可能と判定される

#### Scenario: 無効な正規表現の処理

- **GIVEN** リトライパターンに無効な正規表現 `[unclosed` が含まれる
- **WHEN** エラーパターンマッチングを実行
- **THEN** 無効なパターンは無視される（エラーでクラッシュしない）
- **AND** 他の有効なパターンは正常に評価される

### Requirement: スレッドセーフな実行時刻管理

コマンドキューは複数の非同期タスクから安全に使用できなければならない (MUST)。

実行時刻の管理は以下の通りとする：
- 最後の実行時刻を `Arc<Mutex<Option<Instant>>>` で管理
- コマンド実行前にロックを取得
- 時刻の読み取り・更新を排他的に実行
- ロックは最小限の時間で保持

#### Scenario: 並行コマンド実行時の時刻更新

- **GIVEN** 遅延時間が2秒に設定されている
- **WHEN** 2つのコマンドが同時に実行開始を試みる
- **THEN** 一方のコマンドが先に実行される
- **AND** もう一方のコマンドは2秒遅延後に実行される
- **AND** 実行時刻の更新が競合なく完了する

#### Scenario: 複数の非同期タスクからの利用

- **GIVEN** コマンドキューが複数の非同期タスク間で共有されている
- **WHEN** 異なるタスクから同時にコマンド実行が要求される
- **THEN** すべてのコマンドが適切な遅延を伴って順次実行される
- **AND** データ競合やパニックが発生しない

### Requirement: すべてのコマンド種別への適用

コマンドキューはすべての `*_command` 実行に適用されなければならない (MUST)。

対象となるコマンドは以下の通り：
- `apply_command` - 変更適用
- `archive_command` - 変更アーカイブ
- `resolve_command` - 競合解消・マージ完了
- `analyze_command` - 依存関係分析
- `worktree_command` - ワークツリー上の提案作成
- `acceptance_command` - 受け入れテスト

**実装要件**:
- すべてのAI駆動コマンドは共通ランナー層（`AiCommandRunner`）を経由しなければならない (MUST)
- 時間差起動の状態（`last_execution`）はプロセス全体で共有されなければならない (MUST)
- 並列実行モードの apply/archive も共通ランナー層を経由しなければならない (MUST)
- resolve 実行時に `AgentRunner` を都度作成してはならない (MUST NOT)
- parallel 実行の acceptance は共通の `last_execution` を参照して遅延を適用しなければならない (MUST)

#### Scenario: apply_command での時間差起動とリトライ

- **WHEN** `apply_command` を実行
- **THEN** 時間差起動メカニズムが適用される
- **AND** リトライ可能エラー時に自動リトライが実行される

#### Scenario: resolve_command での優先的リトライ

- **WHEN** `resolve_command` を実行
- **THEN** 時間差起動メカニズムが適用される
- **AND** 競合解消やマージ操作の一時的エラーで自動リトライが実行される

#### Scenario: すべてのコマンドで統一された動作

- **GIVEN** すべての `*_command` が設定されている
- **WHEN** 各コマンドを順次実行
- **THEN** すべてのコマンドで同じキュー設定（遅延時間、リトライ）が適用される
- **AND** コマンド種別による動作の違いがない

#### Scenario: 並列 apply/archive での stagger 適用

- **GIVEN** 並列実行モードで複数の change が処理されている
- **AND** 遅延時間が2秒に設定されている
- **WHEN** worktree A で apply コマンドが実行される
- **AND** 0.5秒後に worktree B で apply コマンドが実行されようとする
- **THEN** worktree B の apply は1.5秒待機してから実行される
- **AND** 両方の apply が共通の `last_execution` 状態を参照している

#### Scenario: resolve での stagger 状態共有

- **GIVEN** 並列実行モードで resolve が必要になった
- **AND** 直前に apply コマンドが実行された
- **WHEN** resolve コマンドが実行されようとする
- **THEN** resolve は apply と同じ `last_execution` 状態を参照する
- **AND** 遅延時間内であれば待機してから実行される

#### Scenario: parallel の apply/archive が CommandQueue 経由で実行される

- **GIVEN** parallel 実行モードで apply/archive が実行される
- **WHEN** apply/archive コマンドが起動される
- **THEN** CommandQueue の stagger と retry が適用される
- **AND** streaming 出力のリトライ通知が既存の出力経路に送信される

#### Scenario: parallel acceptance が共通スタッガーを参照する

- **GIVEN** 並列実行モードで複数の change が acceptance を開始する
- **AND** 遅延時間が2秒に設定されている
- **WHEN** worktree A で acceptance コマンドが起動される
- **AND** 0.5秒後に worktree B で acceptance コマンドが起動されようとする
- **THEN** worktree B の acceptance は1.5秒待機してから実行される
- **AND** 両方の acceptance が共通の `last_execution` 状態を参照している

### Requirement: Streaming 対応リトライ

コマンドキューは streaming 出力を伴うコマンド実行でも、既存のリトライ判定ロジック（エラーパターン、実行時間、exit code）を適用しなければならない (MUST)。

Streaming リトライの動作は以下の通りとする：
- コマンド実行中、stdout/stderr を逐次出力チャネルに送信する
- stderr を同時にバッファリングしてリトライ判定に使用する
- コマンド失敗時、通常のリトライ判定ロジックを適用する
- リトライ時は出力チャネルにリトライ通知を送信する
- 新しいコマンドを spawn して再度 streaming を開始する

#### Scenario: Streaming 実行でリトライが適用される
- **GIVEN** streaming 実行経路でコマンドが失敗する
- **WHEN** exit code が 0 以外で終了する
- **THEN** 既存のリトライ判定ロジックが適用される
- **AND** リトライ通知が出力チャネルに送信される

### Requirement: analyze 出力の厳格検証

`analyze_command` の出力は、exit code が 0 であっても JSON として有効かつ期待スキーマに準拠しなければエラーとしなければならない (MUST)。

期待スキーマは以下の通り：
```json
{
  "groups": [
    { "id": 1, "changes": ["change-a"], "depends_on": [] }
  ]
}
```

検証項目：
1. stdout が JSON としてパース可能であること
2. トップレベルに `groups` キーが存在すること
3. `groups` が配列であること

#### Scenario: exit 0 でも JSON が壊れていたらエラー

- **GIVEN** `analyze_command` が exit code 0 で終了した
- **AND** stdout が有効な JSON ではない（例: 途中で切れた、構文エラー）
- **WHEN** 出力検証が実行される
- **THEN** エラーが返される
- **AND** エラーメッセージに「JSON parse failed」が含まれる
- **AND** stdout の先頭部分がエラーメッセージに含まれる（デバッグ用）

#### Scenario: groups キーが存在しない場合

- **GIVEN** `analyze_command` が exit code 0 で終了した
- **AND** stdout は有効な JSON だが `groups` キーがない（例: `{"result": "ok"}`）
- **WHEN** 出力検証が実行される
- **THEN** エラーが返される
- **AND** エラーメッセージに「missing required key: groups」が含まれる

#### Scenario: 正常な JSON で検証成功

- **GIVEN** `analyze_command` が exit code 0 で終了した
- **AND** stdout が期待スキーマに準拠した JSON である
- **WHEN** 出力検証が実行される
- **THEN** 検証が成功し、パース済みの `AnalysisResult` が返される

### Requirement: 無出力タイムアウトによる中断

コマンドキューは streaming 実行中に stdout/stderr の出力が一定時間発生しない場合、無出力タイムアウトとしてコマンドを中断しなければならない (MUST)。無出力タイムアウトは absolute runtime limit とは独立して評価され、出力行は無出力期限だけを延長し、absolute runtime deadline を延長してはならない (MUST NOT)。

#### Scenario: 無出力が続いた場合はタイムアウトで中断

- **GIVEN** 無出力タイムアウトが 900 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 900 秒以上無出力が継続する
- **THEN** コマンドはタイムアウトとして中断される
- **AND** エラーメッセージに「inactivity timeout」が含まれる

#### Scenario: 出力があれば無出力タイムアウトだけが延長される

- **GIVEN** 無出力タイムアウトが 60 秒に設定されている
- **AND** absolute runtime limit が有効である
- **WHEN** コマンドが 30 秒ごとに stdout を出力する
- **THEN** 無出力タイムアウトは発生しない
- **AND** absolute runtime deadline は延長されない

### Requirement: AI command invocations have an absolute runtime limit

The common AI command runner MUST enforce `command_max_runtime_secs` as an absolute deadline measured from successful child spawn. The default MUST be 10,800 seconds, `0` MUST disable the deadline, and stdout or stderr activity MUST NOT extend it. Runtime-limit expiry MUST close retry admission for the invocation, terminate the owned process group through the existing graceful-then-forceful cleanup path, and return a typed non-retryable runtime-limit outcome.

#### Scenario: Default runtime limit is three hours

**Given**: no configuration layer sets `command_max_runtime_secs`
**When**: the common AI command runner starts an owned command
**Then**: its absolute runtime deadline is 10,800 seconds after successful child spawn

#### Scenario: Continuous output does not extend the absolute deadline

**Given**: `command_max_runtime_secs` is enabled
**And**: an owned AI command emits output continuously
**When**: elapsed time from child spawn reaches the configured limit
**Then**: Conflux closes retry admission for the invocation
**And**: Conflux terminates and proves quiescence for the owned process group
**And**: the command is not automatically retried in the same run

#### Scenario: Zero disables the absolute deadline

**Given**: `command_max_runtime_secs` is `0`
**When**: an owned AI command remains active while satisfying all other lifecycle constraints
**Then**: Conflux does not terminate it solely because of total elapsed runtime
**And**: inactivity timeout and explicit cancellation remain independently enforceable

#### Scenario: Cleanup proof is required after runtime expiry

**Given**: an AI command exceeds its absolute runtime limit
**When**: bounded process-group cleanup cannot prove quiescence
**Then**: Conflux returns actionable cleanup diagnostics
**And**: it does not acknowledge successful termination
**And**: no later retry is admitted for that invocation

### Requirement: Acceptance commands have a dedicated absolute runtime limit

The common AI command runner MUST select an Acceptance-specific absolute runtime limit from the operation type the invocation already declares, without a caller-supplied limit and without changing the runner's signature. `acceptance_max_runtime_secs` MUST default to 1,800 seconds; its validated range and its rejection of zero belong to the configuration capability rather than to this one. When `command_max_runtime_secs` is positive, Acceptance MUST use the minimum of the common and dedicated limits, including common values below the dedicated key's configuration floor. When `command_max_runtime_secs` is zero, Acceptance MUST remain bounded by the dedicated limit. Acceptance output activity MUST NOT extend the deadline. Expiry MUST close retry admission for that invocation, terminate and prove quiescence for the owned process group through the existing cleanup path, and return a typed non-retryable Acceptance runtime failure. That failure MUST NOT enter no-verdict protocol continuation, corrective command-recovery retry, or inactivity-timeout classification. Every other operation type, including cleanup review, MUST retain `command_max_runtime_secs` semantics even when it runs the same configured agent command.

#### Scenario: Acceptance uses the shorter default

**Given**: no configuration layer sets `acceptance_max_runtime_secs`
**And**: the common command runtime default is 10,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline is 1,800 seconds after successful child spawn

#### Scenario: Disabled common limit does not unbound Acceptance

**Given**: `command_max_runtime_secs` is zero
**And**: `acceptance_max_runtime_secs` is 1,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline remains 1,800 seconds after successful child spawn

#### Scenario: Shorter common safety limit still applies

**Given**: `command_max_runtime_secs` is 300 seconds
**And**: `acceptance_max_runtime_secs` is 1,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline is 300 seconds after successful child spawn

#### Scenario: Output does not extend Acceptance runtime

**Given**: an Acceptance command continuously emits output
**When**: elapsed time reaches `acceptance_max_runtime_secs`
**Then**: retry admission closes
**And**: the owned process group is terminated and reaped
**And**: the invocation returns a typed non-retryable Acceptance runtime failure

#### Scenario: The operation type selects the deadline

**Given**: one command runner serves every operation class
**When**: it starts an invocation labelled `acceptance`
**Then**: it applies the dedicated Acceptance limit
**And**: an invocation labelled with any other operation type receives `command_max_runtime_secs`
**And**: no call site supplies a runtime limit of its own

#### Scenario: Other command limits remain unchanged

**Given**: Acceptance and common command runtime limits differ
**When**: Apply and Acceptance commands start
**Then**: Acceptance receives the dedicated limit
**And**: Apply retains `command_max_runtime_secs`

#### Scenario: Cleanup failure is not hidden

**Given**: Acceptance exceeds its absolute runtime limit
**When**: process-group quiescence cannot be proven
**Then**: Conflux reports actionable cleanup failure diagnostics
**And**: it does not acknowledge termination or retry the invocation

#### Scenario: Runtime expiry does not enter other retry protocols

**Given**: Acceptance reaches its absolute runtime limit before producing a canonical verdict
**When**: the owned process group is terminated and reaped
**Then**: Conflux returns the typed Acceptance runtime failure
**And**: it does not enter no-verdict protocol continuation
**And**: it does not enter corrective command-recovery retry
**And**: it is not classified as inactivity timeout
