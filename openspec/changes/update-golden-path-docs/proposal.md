# Change: Golden Path ドキュメント刷新

## Why
README.md と docs/guides/USAGE.md に、現行CLIと一致しないコマンド/フラグが混在しており、初見ユーザーが最初の導線で詰まりやすい。最短導線（Golden Path）を実装に同期して明確化し、オンボーディング成功率を上げる。

## What Changes
- README.md の Quick Start / Usage を現行 CLI の動作に合わせて整理し、Golden Path を明示する
- docs/guides/USAGE.md の例を現行 CLI と一致させ、存在しないコマンド/フラグを排除する
- README.ja.md を README.md と同等の構成・内容に同期する

## Impact
- Affected specs: documentation
- Affected code: README.md, README.ja.md, docs/guides/USAGE.md
