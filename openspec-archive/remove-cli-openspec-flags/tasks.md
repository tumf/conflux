## 1. CLIフラグ削除
- [x] 1.1 `--opencode-path` と `--openspec-cmd` のCLI定義を削除する
- [x] 1.2 該当フラグに依存する引数受け渡しと未使用引数を整理する
- [x] 1.3 CLIテストから `--opencode-path` / `--openspec-cmd` の検証を削除する

## 2. ヘルプ出力の拡張
- [x] 2.1 `cflx --help` に全サブコマンドと主要オプション一覧を追記する
- [x] 2.2 `run` と `tui` の `--web` / `--web-port` / `--web-bind` をヘルプに明示する
- [x] 2.3 `--parallel` / `--max-concurrent` / `--dry-run` / `--vcs` など主要オプションも併記する
- [x] 2.4 ヘルプ出力のテストを追加する

## 3. 仕様更新
- [x] 3.1 `cli` 仕様にフラグ削除とヘルプ拡張の要件を追加する
- [x] 3.2 `configuration` 仕様から `OPENSPEC_CMD` / `--openspec-cmd` の記述を削除する

## 4. 検証
- [x] 4.1 `cargo test` を実行する
- [x] 4.2 `npx @fission-ai/openspec@latest validate remove-cli-openspec-flags --strict` を実行する
