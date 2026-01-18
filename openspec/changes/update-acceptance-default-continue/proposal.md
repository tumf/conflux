# Change: acceptance の未マーカー時は CONTINUE をデフォルトにする

## Why
acceptance 出力に PASS/FAIL/CONTINUE のマーカーが含まれない場合、現状は FAIL として扱われます。この挙動は未確定の調査を継続したいケースでも apply ループに戻されるため、意図せず失敗扱いになるリスクがあります。未マーカー時は CONTINUE とみなし、検証の継続を促す方が運用上安全です。

## What Changes
- acceptance 出力にマーカーが存在しない場合のデフォルト判定を CONTINUE に変更する
- CLI と parallel の acceptance loop に未マーカー時 CONTINUE の取り扱いを明記する
- acceptance_max_continues のリトライ仕様に未マーカー判定を含める

## Impact
- Affected specs: cli, parallel-execution
- Affected code: src/acceptance.rs, acceptance loop (CLI/parallel)
