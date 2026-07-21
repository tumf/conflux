# Design: resolve dependency dispatch gate

## Decision

Resolveはglobal scheduler barrierにしない。依存edge単位のdispatch barrierとして扱う。

`DependencyContext`をdependency target classificationのauthoritative runtime viewとし、queued、in-flight、active、archived、rejected、terminal errorに加えて、resolve lifecycle evidenceを同一snapshotへ含める。Dispatch selectionはanalyzerが返したorderを候補順としてのみ使用し、各candidateのproposal metadata dependencyをrepository-visible effective-base merge evidenceで再検証する。

## State Model

Dependencyは次の条件をすべて満たした場合だけdispatch-satisfiedとなる。

1. rejected、missing、terminal errorではない。
2. active resolve、resolve-wait、in-flight integrationではない。
3. effective dependency baseがdependencyのarchive/merge commitを含む。
4. effective dependency base上でactive change directoryが残っていない。

判定不能、VCS query失敗、lifecycleとrepository evidenceの不整合は未解決として扱う。

## Dispatch Behavior

- Resolving dependency: dependentをblockedとして保持する。
- Resolving unrelated change: capacityがあればcandidateをdispatchする。
- Resolve command completed but merge evidence absent: dependentをblockedとして保持する。
- Resolve integration completed and visible on effective base: resolve completion reanalysis後にdependentをdispatch可能とする。

## Alternatives Rejected

### Resolve中のglobal dispatch停止

安全だが、独立changeの並列性を失い、既存のslot modelを不必要に変更する。

### Analyzer outputだけで依存を判定

Analyzer responseとresolve lifecycleの間にraceがあり、repository-visible merge evidenceより古いまたは不完全な判断をdispatchへ持ち込める。

### Archive directory存在を依存充足とする

archiveはworkspace側で先に成立し得るため、base integration完了の証拠にならない。Canonical specとConstitutionのtruthful completion原則にも反する。

## Verification Strategy

Scheduler integration testでevent sequenceを観測する。Helper単体のclassification testだけでは不十分とし、dependent `ApplyStarted`がmerge前に出ず、unrelated `ApplyStarted`は出て、merge後のreanalysisでdependent `ApplyStarted`が出ることを1つのlifecycle scenarioとして検証する。
