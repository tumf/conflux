## Implementation Tasks

- [x] 1. `ansi-up` パッケージを `dashboard/package.json` の dependencies に追加し `npm install` を実行する (verification: `node -e "require('ansi-up')"` が成功する)
- [x] 2. `dashboard/src/components/LogEntry.tsx` を修正し、`entry.message` を `ansi-up` の `ansi_to_html()` で HTML 変換してから `dangerouslySetInnerHTML` で描画する。`AnsiUp` インスタンスの `escape_for_html` を有効にして XSS を防止する (verification: `dashboard/src/components/LogEntry.tsx` に `AnsiUp` のインポートと `dangerouslySetInnerHTML` が含まれている)
- [x] 3. ANSI カラー出力が暗色テーマで見やすいよう、`ansi-up` の `use_classes` オプションを有効にし、対応する CSS クラスを追加する (verification: `dashboard/src/` 配下に `ansi-*` クラスの CSS 定義が存在する)
- [x] 4. `LogEntry` のユニットテストを作成する: (a) ANSI コード付きメッセージが `<span>` タグ付き HTML に変換される (b) ANSI なしメッセージが通常表示される (c) `<script>` タグがエスケープされる (verification: `cd dashboard && npm test -- --run` が全件 PASS する)
- [x] 5. `cd dashboard && npm run build` が成功することを確認する (verification: ビルド成功、TypeScript エラーなし)

## Future Work

- 256 色 / TrueColor 対応（現在は標準 16 色のみ）
- バックエンド側でオプションとして ANSI strip を提供する設定の追加
