---
title: Conflux 側の根本対策 (TTY停止/STAT=T 対策)
status: approved
---

# Change: conflux-root-tty-hardening

## 背景

Claude Code (`claude --output-format stream-json`) を `cflx` がパイプライン（例: `sh -c "... | ..."`）として起動する際、実行途中にプロセスが `STAT=T`（stop）となり、出力が途中で止まってストールしたように見えることがある。

この症状はジョブ制御（SIGTTIN/SIGTTOU 等）により、フォアグラウンドではないプロセスグループが TTY にアクセスしようとした場合に発生し得る。

## 目的

- `cflx` が起動する子プロセスを、実行経路に関係なく「非対話・TTY 非依存」に寄せる。
- Unix 系では必ず制御 TTY から切り離し（新セッション化）を試み、ジョブ制御による stop を回避する。

## スコープ

- `src/agent/runner.rs` の `build_command()` / `build_command_in_dir()` の Unix 側 pre-exec を、
  `setpgid()` 固定から `process_manager::configure_process_group()`（`setsid()` 優先、失敗時 `setpgid()`）に切り替える。
- 既存の `stdin=null` / `stdout,stderr=piped` の方針は維持する。

## 非目標

- Claude CLI そのものの挙動変更（外部要因）。
- `cc-stream` 等の外部ラッパーへの変更（本変更は Conflux 本体のみ）。
