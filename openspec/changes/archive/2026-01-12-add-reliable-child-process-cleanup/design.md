# Design: 子プロセスの確実なクリーンアップ

## Context

現在のプロセス管理の課題：

1. **Unix 系**: `setsid()` で新セッションを作成しているが、プロセスグループ全体を kill する仕組みがない
2. **Windows**: 標準的なジョブオブジェクトを使っていないため、親終了時に子が残る
3. **run モード**: シグナルハンドリングがなく、Ctrl+C で終了した場合に子プロセスが残る
4. **TUI モード**: 2秒のタイムアウトで終了するが、クリーンアップが間に合わない可能性がある

## Goals / Non-Goals

### Goals
- Unix 系とWindows の両方で標準的なプロセス管理手法を採用
- アプリケーション終了時（正常終了/シグナル/クラッシュ）に子プロセスが確実に終了
- 既存の機能を壊さない（後方互換性維持）

### Non-Goals
- 子プロセスのリソース使用量監視
- タイムアウトベースの強制終了（既存の cancel_token メカニズムを維持）
- 孫プロセス以降の追跡（直接の子プロセスのみ対象）

## Decisions

### 1. Unix 系: Process Group による管理

**決定**: `setpgid(0, 0)` で新プロセスグループを作成し、終了時に `killpg()` でグループ全体を kill

**理由**:
- `setsid()` は新セッションを作成するが、セッションリーダーを kill しても子プロセスは残る
- `setpgid()` + `killpg()` はプロセスグループ全体を管理する標準的な手法
- シェル経由で起動する場合も、シェル配下のプロセスグループごと終了できる

**実装**:
```rust
#[cfg(unix)]
cmd.pre_exec(|| {
    use nix::unistd::{setpgid, Pid};
    // 自身を新プロセスグループのリーダーにする
    setpgid(Pid::from_raw(0), Pid::from_raw(0))?;
    Ok(())
});

// kill 時
#[cfg(unix)]
fn kill_process_group(child: &Child) -> io::Result<()> {
    if let Some(pid) = child.id() {
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;
        // プロセスグループ全体に SIGTERM を送信
        killpg(Pid::from_raw(pid as i32), Signal::SIGTERM)?;
    }
    Ok(())
}
```

**代替案と却下理由**:
- `setsid()` のまま維持 → セッション配下のプロセスを列挙する必要があり複雑
- `kill(-pid, SIGTERM)` → `killpg()` のラッパーだが、nix クレートの方が型安全

### 2. Windows: Job Object による管理

**決定**: `JobObject` を作成し、子プロセスを割り当て、JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE フラグで自動終了

**理由**:
- Windows の標準的なプロセス管理手法
- 親プロセスが異常終了しても、ジョブオブジェクトのハンドルが閉じられると自動的に子プロセスが終了
- ジョブ配下のプロセスツリー全体を管理できる

**実装**:
```rust
#[cfg(windows)]
use windows::Win32::System::JobObjects::*;

#[cfg(windows)]
struct JobObjectGuard {
    handle: HANDLE,
}

impl Drop for JobObjectGuard {
    fn drop(&mut self) {
        // ハンドルが閉じられると、子プロセスも終了する
        unsafe { CloseHandle(self.handle); }
    }
}

// spawn 時
#[cfg(windows)]
fn assign_to_job(child: &Child) -> io::Result<JobObjectGuard> {
    let job = unsafe { CreateJobObjectW(None, PCWSTR::null())? };

    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

    unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;

        let process_handle = OpenProcess(
            PROCESS_ALL_ACCESS,
            false,
            child.id().unwrap(),
        )?;

        AssignProcessToJobObject(job, process_handle)?;
        CloseHandle(process_handle)?;
    }

    Ok(JobObjectGuard { handle: job })
}
```

**代替案と却下理由**:
- `taskkill /F /T /PID` → 外部コマンドに依存し、エラーハンドリングが複雑
- プロセスツリーを手動で列挙 → 競合状態が発生しやすく、実装が複雑

### 3. run モードのシグナルハンドリング

**決定**: `tokio::signal::ctrl_c()` と `tokio::signal::unix::signal(SIGTERM)` を使用し、受信時に子プロセスをクリーンアップ

**理由**:
- Tokio の標準的な非同期シグナルハンドリング
- 既存の `CancellationToken` と統合しやすい
- クロスプラットフォーム（Windows は Ctrl+C のみ、Unix は SIGINT/SIGTERM）

**実装**:
```rust
// main.rs の run モード内
let cancel_token = CancellationToken::new();
let cancel_for_signal = cancel_token.clone();

tokio::spawn(async move {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received SIGINT, shutting down...");
            cancel_for_signal.cancel();
        }
        #[cfg(unix)]
        _ = async {
            let mut sigterm = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate()
            ).unwrap();
            sigterm.recv().await
        } => {
            info!("Received SIGTERM, shutting down...");
            cancel_for_signal.cancel();
        }
    }
});

// orchestrator.run() に cancel_token を渡し、定期的にチェック
```

**代替案と却下理由**:
- `ctrlc` クレート → Tokio と統合しにくい
- シグナルハンドラなし → 現状維持だが、子プロセスが残る問題が解決しない

### 4. TUI モードの終了待機

**決定**: 終了時のタイムアウトを 2秒 → 5秒に延長し、`child.wait()` で確実に終了を待つ

**理由**:
- 現在の2秒は短すぎる（特にWindows環境）
- `tokio::time::timeout` を使うことで、ハングを防ぎつつ確実に待機
- プロセスグループ/ジョブオブジェクトと組み合わせることで、ほぼ確実に終了

**実装**:
```rust
// tui/runner.rs
if let Some(cancel) = orchestrator_cancel {
    cancel.cancel();
}

if let Some(handle) = orchestrator_handle {
    match tokio::time::timeout(Duration::from_secs(5), handle).await {
        Ok(_) => info!("Orchestrator task finished gracefully"),
        Err(_) => warn!("Orchestrator task timeout after 5 seconds"),
    }
}
```

## Risks / Trade-offs

### リスク 1: プロセスグループによる副作用
- **リスク**: シェル配下で起動したプロセスが意図せず終了する可能性
- **軽減策**: `setpgid(0, 0)` により、親のプロセスグループから分離するため影響は限定的
- **受容**: agent コマンドが起動する子プロセスは orchestrator の責任範囲

### リスク 2: Windows ジョブオブジェクトの互換性
- **リスク**: 古い Windows バージョンでサポートされていない可能性
- **軽減策**: Windows 7 以降で安定サポート、失敗時は従来の `kill()` にフォールバック
- **受容**: サポート対象 OS を Windows 10 以降とする（既存の前提）

### リスク 3: クリーンアップ時間の増加
- **リスク**: 終了時に最大5秒待機するため、応答性が低下
- **軽減策**: ほとんどの場合は即座に終了し、タイムアウトは最悪ケースのみ
- **受容**: 確実性と応答性のトレードオフで、確実性を優先

## Migration Plan

1. **Phase 1**: Unix系のプロセスグループ対応（`nix` クレート追加）
2. **Phase 2**: Windows のジョブオブジェクト対応（`windows` クレート追加）
3. **Phase 3**: run モードのシグナルハンドリング追加
4. **Phase 4**: TUI モードの終了待機時間調整

各フェーズは独立しており、段階的にロールアウト可能。

## Open Questions

なし（要件は明確）
