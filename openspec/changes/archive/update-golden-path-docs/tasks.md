## 1. Golden Path の定義と反映
- [x] 1.1 README.md の Quick Start を現行 CLI の実態に合わせて再構成する（`cflx` TUI / `cflx init` / `cflx run` を最短導線として整理し、存在しないコマンド/フラグを除去する）
- [x] 1.2 docs/guides/USAGE.md の例を整理し、`cflx status`/`cflx reset`/`--openspec-path` など実装に存在しない項目を排除し、`cflx --help` と整合させる
- [x] 1.3 README.ja.md を README.md と同一構成・同一コマンド例に同期する（翻訳以外の差分をなくす）

## 2. 検証
- [x] 2.1 README.md / README.ja.md / docs/guides/USAGE.md で記載されているコマンド・フラグが `cflx --help` の内容と一致していることを確認する（更新済みの各ファイルを目視で照合する）

## Acceptance #1 Failure Follow-up
- [x] README.ja.md を README.md と完全に同期する（翻訳差分のみ）。不足している `## Documentation` セクション、Features の Web Monitoring 項目、Web Monitoring の REST API (`/api/changes/{id}/approve`, `/api/changes/{id}/unapprove`)、および Configuration 例/プレースホルダー表の `acceptance_command`・`resolve_command`・`acceptance_prompt`・`acceptance_prompt_mode` 関連記述を追加する。
