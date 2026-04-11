## ADDED Requirements

### Requirement: config-loading-errors-safe-propagation

設定データのパースや読み込み中の問題はパニックを起こさず、呼び出し元に適切なエラー型として伝播しなければならない。

#### Scenario: invalid-config-returns-error-with-context

**Given**: `src/config/mod.rs` 内の設定ファイルロード処理が不正なフォーマットまたは不正な型を含む入力を受け取る
**When**: 設定ロードメソッドを実行する
**Then**: `unwrap`/`expect` によるパニックを起こさず `Err` を返す
**And**: エラーメッセージには問題のある設定キーまたはファイルパスの文脈が含まれる

#### Scenario: io-read-failure-returns-typed-error

**Given**: 設定ファイルパスが存在しない、または読み取り不可能である
**When**: 設定ロードメソッドを実行する
**Then**: プロセスをクラッシュさせずに I/O 由来の型付きエラーを返す
**And**: 呼び出し元がエラーをハンドリング可能である

#### Scenario: missing-required-key-does-not-panic

**Given**: 必須キーが欠落した `conflux.toml` を読み込む
**When**: `Config::load` を呼び出す
**Then**: ロード処理はパニックせず、設定不備を示すエラーを返す
**And**: プログラムは異常終了しない
