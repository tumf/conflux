## Specification Tasks

- [ ] `frontend-abstraction/spec.md` に Core / Frontend 責務境界の総則 requirement を追加 (expected: Core 所有状態・Frontend 所有状態・Frontend 禁止状態が ADDED requirement として定義される)
- [ ] `orchestration-state/spec.md` の既存「Resolve Wait Queue Ownership」要件に Core 所有の注記を追加 (expected: resolve queue / serialization が Core 所有であり Frontend がローカルコピーを持ってはならないことが明記される)
- [ ] 全 spec delta を validate --strict で検証 (expected: validation passed)

## Future Work

- `tui-architecture/spec.md` 内の「TUI SHALL maintain a FIFO resolve wait queue」を Core 所有に沿った表現に修正
- `tui-resolve-queue/spec.md` の「TUI-local state と shared reducer の両方を同期」を Core 正規・Frontend キャッシュに修正
- Web 側仕様が既に Core 派生であることを確認的に明記
