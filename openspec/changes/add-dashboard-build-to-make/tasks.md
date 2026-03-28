## Implementation Tasks

- [ ] Makefile にフロントエンドビルド用の共通ターゲットまたは共通コマンド列を追加する (verification: `Makefile` に `dashboard/build.sh` または同等処理を再利用する前処理がある)
- [ ] `build` ターゲットを更新し、フロントエンドビルド成功後に `cargo build --release` を実行する (verification: `Makefile` の `build` ターゲット定義)
- [ ] `install` ターゲットを更新し、フロントエンドビルド成功後に `cargo install --path .` を実行する (verification: `Makefile` の `install` ターゲット定義)
- [ ] `build-linux-x86` と `build-linux-arm` を更新し、クロスビルド前に同じフロントエンドビルド前処理を実行する (verification: `Makefile` の各ターゲット定義)
- [ ] フロントエンド前処理込みで `make build` を実行し、ダッシュボードビルド後に Rust リリースビルドが通ることを確認する (verification: `make build` 実行ログ)
- [ ] lint と typecheck 相当の既存コマンドを実行して、変更後もリポジトリの品質チェックが通ることを確認する (verification: `cargo clippy -- -D warnings` と `cargo test` または既存の検証コマンド)

## Future Work

- 必要であれば `clean` ターゲットに `dashboard/dist` の削除を追加する
- 必要であれば `build.rs` の `rerun-if-changed` 設計を見直し、Make 非依存でも最新アセットを保証できるようにする
