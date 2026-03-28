## ADDED Requirements

### Requirement: project-file-browsing-api

サーバーはダッシュボード用に、プロジェクトのworktree配下のファイルツリーとファイル内容を読み取り専用で提供しなければならない。

#### Scenario: list-file-tree-from-base-worktree

**Given**: プロジェクト `p1` が登録されている
**When**: `GET /api/v1/projects/p1/files/tree?root=base` が呼ばれる
**Then**: ベースworktreeのルートから再帰的なファイルツリーをJSONで返す
**And**: `.git`, `node_modules`, `target`, `.next`, `dist` ディレクトリは除外される

#### Scenario: list-file-tree-from-specific-worktree

**Given**: プロジェクト `p1` にworktree `cflx/add-feature` が存在する
**When**: `GET /api/v1/projects/p1/files/tree?root=worktree:cflx/add-feature` が呼ばれる
**Then**: 指定worktreeのルートから再帰的なファイルツリーをJSONで返す

#### Scenario: read-file-content

**Given**: プロジェクト `p1` のベースworktreeに `openspec/changes/add-feature/proposal.md` がある
**When**: `GET /api/v1/projects/p1/files/content?root=base&path=openspec/changes/add-feature/proposal.md` が呼ばれる
**Then**: レスポンスはファイルのテキスト内容を返す

#### Scenario: reject-path-traversal

**Given**: プロジェクト `p1` が登録されている
**When**: パスに `..` を含むファイル内容取得APIが呼ばれる
**Then**: サーバーは 400 系エラーを返す
**And**: worktreeルートの外側のファイルを読み取らない

#### Scenario: truncate-large-file-content

**Given**: プロジェクト `p1` のworktreeに1MBを超えるファイルがある
**When**: そのファイルの内容取得APIが呼ばれる
**Then**: レスポンスは上限までの内容を返し、truncatedフラグを含む

#### Scenario: detect-binary-file

**Given**: プロジェクト `p1` のworktreeにバイナリファイルがある
**When**: そのファイルの内容取得APIが呼ばれる
**Then**: レスポンスはファイル内容を返さず、binaryフラグを含む
