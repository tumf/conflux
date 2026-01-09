# Change: unicode-width クレートによる表示幅計算の改善

## Why

TUI のログパネルで日本語などのマルチバイト文字を含むメッセージをトランケートする際、バイト境界の問題で panic が発生した（`byte index 212 is not a char boundary`）。

現在の修正では `chars().count()` で文字数ベースに変更したが、日本語文字は表示幅が2カラムであるため、実際の表示幅と一致しない。`unicode-width` クレートを導入し、正確な表示幅計算を行う。

## What

- `unicode-width` クレートを依存関係に追加
- TUI のテキストトランケート処理を Unicode 表示幅ベースに変更
- 文字列の表示幅を正確に計算し、ターミナル幅に収まるようにトランケート

## Scope

- **In scope**: TUI のログパネルのトランケート処理改善
- **Out of scope**: 他のパネル（変更リストなど）の表示幅対応は別提案で検討

## Impact

- **Low risk**: 依存関係の追加と内部処理の改善のみ
- **No breaking changes**: 外部 API や設定への影響なし
