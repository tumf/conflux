# Change: Web UIのchange statusをTUIに合わせる

## Why
Web UIのchange status表示と集計がTUIのQueueStatusと一致しておらず、実行状況の解釈に差異が生じています。TUIと完全に同じステータス語彙で表示・集計できるようにし、監視画面としての信頼性を高めます。

## What Changes
- Web UIのステータス表示をTUIのQueueStatus表記に統一する
- Web UIの統計・集計ロジックをQueueStatus基準に揃える
- legacyなpending/in_progress/complete表示のフォールバックを排除する
- TUIに存在するAcceptingステータスのWeb UI表示を定義する

## Impact
- Affected specs: web-monitoring
- Affected code: web/app.js, web/style.css
