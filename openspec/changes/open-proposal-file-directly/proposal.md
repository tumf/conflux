# TUIエディタ起動時にproposal.mdを直接開く

## 概要

TUIで`e`キーを押したときに、changeディレクトリ全体ではなく`proposal.md`ファイルを直接開くように変更する。`proposal.md`が存在しない場合は、従来通りディレクトリを開く（フォールバック動作）。

## Why

現在、TUIで`e`キーを押すと、エディタが`openspec/changes/{change_id}/`ディレクトリで起動し、`.`（カレントディレクトリ）が開かれる。しかし、ほとんどの場合ユーザーが編集したいのは`proposal.md`ファイルであり、ディレクトリを開いた後にファイルをナビゲートする必要がある。

この追加のナビゲーション手順は以下の問題を引き起こす：
- **時間の無駄**: 毎回エディタ内でproposal.mdを探す必要がある
- **認知的負荷**: ユーザーは「proposal.mdを編集したい」という明確な意図を持っているのに、余分な操作が必要
- **一貫性の欠如**: ユーザーの主な編集対象（proposal.md）に直接アクセスできない

この変更により、最も頻繁に使用されるワークフロー（proposal.mdの編集）を1ステップで実現し、ユーザーエクスペリエンスを向上させる。

## 提案する変更

### 動作の変更

**現在の動作:**
1. `e`キーを押す
2. エディタが`openspec/changes/{change_id}/`で起動
3. 引数として`.`が渡される
4. ユーザーがエディタ内で`proposal.md`を探して開く

**新しい動作:**
1. `e`キーを押す
2. `openspec/changes/{change_id}/proposal.md`の存在を確認
3. ファイルが存在する場合: `proposal.md`を直接開く
4. ファイルが存在しない場合: 従来通りディレクトリを開く（フォールバック）

### メリット

- **効率向上**: 最も頻繁に編集するファイルに直接アクセス
- **ワークフロー改善**: エディタ起動後のナビゲーションが不要
- **後方互換性**: `proposal.md`がない場合はディレクトリにフォールバック

### 実装範囲

- `src/tui/utils.rs`の`launch_editor_for_change()`関数を修正
- フォールバック動作の追加（proposal.mdがない場合はディレクトリを開く）
- ログメッセージの更新
- 仕様の更新

## 影響範囲

### 変更対象ファイル

- `src/tui/utils.rs`: エディタ起動ロジックの変更
- `openspec/specs/tui-editor/spec.md`: 仕様の更新

### 影響を受ける機能

- TUIの`e`キーによるエディタ起動機能のみ
- 他の機能への影響なし

## 技術的詳細

### 実装アプローチ

```rust
// 疑似コード
pub fn launch_editor_for_change(change_id: &str) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    
    let proposal_path = Path::new("openspec/changes")
        .join(change_id)
        .join("proposal.md");
    let change_dir = Path::new("openspec/changes").join(change_id);
    
    // proposal.mdが存在する場合は直接開く、なければディレクトリにフォールバック
    if proposal_path.exists() {
        // proposal.mdを開く
        Command::new(&editor)
            .arg(&proposal_path)
            .status()?;
    } else if change_dir.exists() {
        // フォールバック: ディレクトリを開く
        Command::new(&editor)
            .arg(".")
            .current_dir(&change_dir)
            .status()?;
    } else {
        return Err(OrchestratorError::ChangeNotFound(change_id.to_string()));
    }
    
    Ok(())
}
```

### エッジケース

1. **proposal.mdが存在しない**: ディレクトリにフォールバック（従来の動作）
2. **changeディレクトリが存在しない**: エラーを返す（現在と同じ）
3. **エディタコマンドが失敗**: エラーを返す（現在と同じ）

## 検証方法

### テストシナリオ

1. **proposal.mdが存在する場合**: ファイルが直接開かれることを確認
2. **proposal.mdが存在しない場合**: ディレクトリが開かれることを確認
3. **changeディレクトリが存在しない場合**: 適切なエラーが表示されることを確認
4. **様々なエディタ**: vi, vim, nvim, VS Code等で動作確認

### 手動テスト

```bash
# 1. TUIを起動
cargo run -- tui

# 2. changeにカーソルを合わせて`e`を押す

# 3. proposal.mdが直接開かれることを確認

# 4. proposal.mdを削除して再度テスト（ディレクトリが開かれることを確認）
```

## 将来の拡張可能性

この変更により、将来的に以下のような拡張が可能になる：

- 他のキー（例: `t`で`tasks.md`を開く）の追加
- 設定ファイルでデフォルトファイルをカスタマイズ
- 複数ファイルの同時オープン

## リスクと制約

### リスク

- **低リスク**: 変更範囲が1つの関数に限定され、フォールバック動作により既存の動作を保持
- エディタの種類による動作の違い（一部のエディタはファイルパスを受け取れない可能性）

### 制約

- `proposal.md`が標準的なファイル名であることに依存
- OpenSpecの規約に従ったchangeディレクトリ構造を前提とする

## 承認基準

- [ ] `cargo test`が成功すること
- [ ] `cargo fmt`と`cargo clippy`が警告なしで通ること
- [ ] 手動テストでproposal.mdが直接開かれることを確認
- [ ] 手動テストでフォールバック動作（proposal.mdなし）を確認
- [ ] 仕様の更新が完了していること
- [ ] `npx @fission-ai/openspec@latest validate open-proposal-file-directly --strict`が成功すること
