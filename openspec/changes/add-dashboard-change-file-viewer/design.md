## Context

ダッシュボードの右ペイン（LOGSエリア）にファイルビューアを追加する。ファイルビューアはプロジェクトのworktree全体をルートとしたファイルツリーを表示し、コンテキスト（Change or Worktree）に応じて適切なノードを自動展開する。

## Goals / Non-Goals

- Goals:
  - プロジェクトのworktreeルートからファイルツリーを表示する
  - Change選択時に `openspec/changes/<change_id>/` パスを自動展開し、`proposal.md` を自動オープンする
  - Worktree選択時にそのworktreeのルートからファイルツリーを表示する
  - ファイル内容をプレーンテキスト（font-mono, 行番号付き）で閲覧可能にする

- Non-Goals:
  - ファイルの編集・保存機能（読み取り専用）
  - Markdownレンダリングやシンタックスハイライト（初期版はプレーンテキスト表示）
  - リアルタイムファイル変更監視

## Decisions

### ファイルビューアのコンテキストモデル

ファイルビューアは「どのファイルシステムルートを表示するか」を決定する **ファイルブラウズコンテキスト** を持つ。コンテキストは以下の2つの起点から設定される：

1. **Change選択**: Changesパネルの行クリック → ベースworktreeルートを表示、`openspec/changes/<change_id>/` を自動展開、`proposal.md` を自動表示
2. **Worktree選択**: Worktreesパネルの行クリック → そのworktreeパスをルートとして表示

フロントエンド状態:
```typescript
interface FileBrowseContext {
  type: 'change' | 'worktree';
  // Change: { changeId, expandPath: "openspec/changes/<id>", autoOpenFile: "openspec/changes/<id>/proposal.md" }
  // Worktree: { worktreePath, branch }
  changeId?: string;
  worktreeBranch?: string;
}
```

### バックエンドAPI設計

worktreeルートやサブパスに対して汎用的にファイルツリー・内容を返す2本のAPI：

#### ファイルツリー API
```
GET /api/v1/projects/:id/files/tree?root=base|worktree:<branch>
```
- `root=base` (default): プロジェクトのベースworktreeルート (`data_dir/worktrees/<project_id>/<branch>`)
- `root=worktree:<branch_name>`: 指定worktreeのルート
- レスポンス: 再帰的ファイルツリーJSON
- `.git` ディレクトリ, `node_modules`, `.next` 等は除外

#### ファイル内容 API
```
GET /api/v1/projects/:id/files/content?root=base|worktree:<branch>&path=<relative_path>
```
- `path`: ルートからの相対パス
- セキュリティ: パストラバーサル防止（`..` を含むパスは400エラー）
- サイズ上限: 1MB超は切り詰め + truncatedフラグ
- バイナリ判定: NULバイトがある場合は内容を返さずバイナリフラグを返す

### Change選択の仕組み

- `useAppStore` に `fileBrowseContext: FileBrowseContext | null` を追加
- `ChangeRow` のチェックボックス以外の領域クリックで `setFileBrowseContext({ type: 'change', changeId })` を設定
- `WorktreeRow` のアクションボタン以外の領域クリックで `setFileBrowseContext({ type: 'worktree', worktreeBranch })` を設定
- Filesタブの表示内容は `fileBrowseContext` に基づいて決定

### ツリーの自動展開ロジック

Change選択時:
1. ツリー取得後、`openspec/changes/<change_id>/` までのパスを自動展開状態にする
2. `openspec/changes/<change_id>/proposal.md` が存在すれば自動選択して内容を表示

### UIレイアウト

デスクトップ右ペイン:
```
┌──────────────────────────────┐
│  [Logs]  [Files]    ← タブ   │
├──────────────────────────────┤
│ (Filesタブ:)                 │
│ ┌────────┬───────────────┐   │
│ │ツリー   │ファイル内容     │   │
│ │(~200px)│               │   │
│ │📁openspec/             │   │
│ │ 📁changes/             │   │
│ │  📁add-feat/ ← 展開     │   │
│ │   📄proposal.md ← 選択  │   │
│ │   📄tasks.md            │   │
│ │   📁specs/              │   │
│ │📁src/                   │   │
│ │📄Cargo.toml             │   │
│ └────────┴───────────────┘   │
└──────────────────────────────┘
```

### 除外パターン

ファイルツリーから除外するディレクトリ:
- `.git`
- `node_modules`
- `.next`
- `target` (Rust build)
- `dist`

## Risks / Trade-offs

- 大きなプロジェクトのツリー全体を返すとレスポンスが大きくなる → ディレクトリ除外パターンで軽減、将来的にはlazy loadingも検討
- パストラバーサル攻撃 → パスバリデーション必須
