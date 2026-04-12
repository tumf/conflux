# QUICKSTART

[![日本語](https://img.shields.io/badge/日本語-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ja.md) [![English](https://img.shields.io/badge/English-QUICKSTART-blue?style=flat-square)](./QUICKSTART.md) [![简体中文](https://img.shields.io/badge/简体中文-QUICKSTART-blue?style=flat-square)](./QUICKSTART.zh-CN.md) [![Español](https://img.shields.io/badge/Español-QUICKSTART-blue?style=flat-square)](./QUICKSTART.es.md) [![Português%20(BR)](https://img.shields.io/badge/Português%20(BR)-QUICKSTART-blue?style=flat-square)](./QUICKSTART.pt-BR.md) [![한국어](https://img.shields.io/badge/한국어-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ko.md) [![Français](https://img.shields.io/badge/Français-QUICKSTART-blue?style=flat-square)](./QUICKSTART.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-QUICKSTART-blue?style=flat-square)](./QUICKSTART.de.md) [![Русский](https://img.shields.io/badge/Русский-QUICKSTART-blue?style=flat-square)](./QUICKSTART.ru.md) [![Tiếng%20Việt](https://img.shields.io/badge/Tiếng%20Việt-QUICKSTART-blue?style=flat-square)](./QUICKSTART.vi.md)

이 문서는 처음으로 `cflx`를 설치하고, 프로젝트를 설정하고, OpenSpec change를 만든 뒤, TUI에서 구현을 끝까지 완료하는 가장 빠른 가이드입니다.

Conflux는 `cflx` 명령으로 구현되어 있습니다.

## 0. 준비 사항

- Rust / Cargo를 사용할 수 있어야 함: [Rust 설치](https://rust-lang.org/tools/install/)
- [Claude Code](https://claude.com/product/claude-code)를 사용할 수 있어야 함
- `~/myproject`와 같은 git 관리 프로젝트가 있어야 함

> Conflux는 AI 에이전트를 실행하고 제어하는 오케스트레이터입니다. 자체적으로 코딩 에이전트는 아닙니다.
> [Claude Code](https://claude.com/product/claude-code), [OpenCode](https://opencode.ai/), [Codex](https://developers.openai.com/codex/cli) 같은 CLI를 사용할 수 있습니다.
> 이 QUICKSTART에서는 Claude Code를 예시로 설명합니다.

준비 사항 확인:

```bash
cargo --version
claude --version
claude -p 'hi'
```

## 1. `cflx` 설치

crates.io에서 설치합니다.

```bash
cargo install cflx
```

설치 후 확인:

```bash
cflx --version
```

## 2. 프로젝트 준비

이후부터는 프로젝트 디렉터리에서 작업합니다. 예시로 `~/myproject`를 사용합니다.

Conflux는 `git worktree`를 사용하므로 프로젝트는 git으로 관리되어야 합니다.

```bash
cd ~/myproject
```

새 프로젝트라면:

```bash
mkdir -p ~/myproject
cd ~/myproject
git init
```

## 3. bundled skills 설치

Claude Code용 Conflux bundled skill을 프로젝트에 추가합니다.

```bash
cflx install-skills --claude
```

그러면 `./.claude/skills` 아래에 `cflx-*` skill이 설치됩니다.

Git에 포함할지는 `.cflx.jsonc`와 함께 나중에 한 번에 결정하면 됩니다.

## 4. 설정 파일 만들기

설정 파일 이름은 `.cflx.conf`가 아니라 `.cflx.jsonc`입니다.

가장 빠른 방법은 템플릿을 생성하는 것입니다.

```bash
cflx init
```

그러면 현재 디렉터리에 `.cflx.jsonc`가 생성됩니다.

## 5. `.cflx.jsonc` 확인

최소한 사용하려는 에이전트용 명령이 들어 있으면 동작합니다.

Claude Code 템플릿 예시:

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}"
}
```

처음 한 번은 `cflx init`이 생성한 내용을 그대로 사용해도 충분합니다.

## 6. Git에 넣을 것 결정

초기 설정에서는 다음 두 가지를 Git에 넣을지 결정합니다.

- `./.claude/skills/cflx-*`
- `./.cflx.jsonc`

권장 사항:

- 팀이나 여러 머신에서 같은 동작을 재현하고 싶다면 둘 다 커밋
- 로컬 전용이고 일회성에 가깝다면 둘 다 `.gitignore`에 추가

판단이 어렵다면 우선 둘 다 커밋해도 괜찮습니다. `.cflx.jsonc`에 비밀 정보를 직접 쓰지 않는 방식으로 운영하면 다루기 쉽습니다.

둘 다 `.gitignore`에 추가하는 경우:

```bash
printf ".claude/skills/cflx-*\n.cflx.jsonc\n" >> .gitignore
git add .gitignore
git commit -m 'Ignore Conflux local setup files'
```

둘 다 저장소에 추가하는 경우:

```bash
git add .claude/skills/cflx-* .cflx.jsonc
git commit -m 'Add Conflux setup files'
```

## 7. 첫 change proposal 만들기

Conflux는 OpenSpec의 change를 처리합니다.

아직 OpenSpec에 익숙하지 않아도 괜찮습니다. bundled skill이 이미 설치되어 있으므로 Claude Code에게 proposal 생성을 맡길 수 있습니다.

예를 들어:

```text
/cflx-proposal python으로 hello world 출력하기
```

그러면 `openspec/changes/add-hello-world/`와 같은 change 디렉터리가 생성되고, 최소한 다음 두 파일이 들어 있습니다.

- `proposal.md`: 무엇을 바꿀지
- `tasks.md`: 무엇을 구현할지

가장 빠르게 진행하려면 이 두 파일을 대략 확인한 뒤 문제가 없으면 그대로 커밋하면 됩니다.

확인 포인트:

- `proposal.md` 내용이 원하는 변경인지
- `tasks.md`의 구현 작업이 과하지도 부족하지도 않은지
- 불필요한 변경이 섞여 있지 않은지

필요하면 proposal이나 tasks를 수정하고, 내용에 문제가 없으면 커밋합니다.

```bash
git add openspec/changes/add-hello-world
git commit -m 'proposal: add-hello-world'
```

자세한 구조는 다음과 같습니다.

```text
openspec
└── changes
    └── add-hello-world
        ├── proposal.md
        ├── specs
        │   └── hello-world
        │       └── spec.md
        └── tasks.md
```

## 8. 워크스페이스가 깨끗한지 확인

TUI를 실행하기 전에 작업 트리가 깨끗한지 확인합니다.

```bash
git status
```

깨끗하다면 다음과 같이 표시됩니다.

```text
On branch main
nothing to commit, working tree clean
```

## 9. TUI 실행

TUI 모드로 Conflux를 실행합니다.

```bash
cflx
```

다음과 같은 화면이 표시됩니다.

![Conflux TUI ready screen](docs/images/quickstart/tui-ready-screen.png)

## 10. TUI에서 실행하기

기본 조작:

- `↑/↓` 또는 `j/k`: change 선택
- `Space`: 실행 대상으로 표시
- `F5`: 실행 시작
- `Ctrl+C`: 종료

가장 짧은 흐름:

1. `cflx` 실행
2. 처리할 change로 이동
3. `Space`로 선택
4. `F5`로 실행

이번 예시에서는 change가 하나뿐이므로 `Space` → `F5`로 실행합니다.

![Conflux TUI running screen](docs/images/quickstart/tui-running-screen.png)

Conflux는 다음 루프를 자동으로 수행합니다.

- apply
- accept
- archive
- resolve / merge

`merged` 상태가 되면 완료입니다.

![Conflux TUI merged screen](docs/images/quickstart/tui-merged-screen.png)

## 11. 결과 확인

구현이 들어갔는지 확인합니다.

```bash
tree
cat hello.py
```

예시:

```text
.
├── hello.py
└── openspec
    ├── changes
    └── specs
```

```python
print("hello world")
```

OpenSpec 쪽도 업데이트됩니다.

```bash
tree openspec -L 10
```

예시:

```text
openspec
├── changes
│   └── archive
│       └── add-hello-world
│           ├── proposal.md
│           ├── specs
│           │   └── hello-world
│           │       └── spec.md
│           └── tasks.md
└── specs
    └── hello-world
        └── spec.md
```

change proposal이 archive 되었고, 최종 사양이 `openspec/specs`로 승격된 것을 확인할 수 있습니다.

예를 들어:

```bash
cat openspec/specs/hello-world/spec.md
```

```markdown
## Requirements

### Requirement: hello-world-output

The program must print "hello world" to standard output when executed.

#### Scenario: default-execution

**Given**: The user has Python installed
**When**: The user runs `python hello.py`
**Then**: The program prints `hello world` to stdout and exits with code 0
```

이 spec이 있으면 Conflux는 소프트웨어의 동작을 빠르게 이해하고, 다음 변경도 더 안정적으로 진행할 수 있습니다.

---

이로써 가장 단순한 구현 사이클이 완료되었습니다.

이 QUICKSTART는 첫 실행을 가장 빠르게 통과하는 지점까지만 다룹니다.
실제 운영에서는 proposal 다듬기, 설정 조정, 병렬 실행, 트러블슈팅 등 더 세밀한 기법이 필요할 수 있습니다.
이후 내용은 README나 `cflx --help`를 참고하세요.

의견이나 질문은 [GitHub Issue](https://github.com/tumf/conflux/issues)에 남기거나 X에서 `@tumf`를 멘션해 주세요.
