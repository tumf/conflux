# Conflux

[![日本語](https://img.shields.io/badge/%E8%A8%80%E8%AA%9E-日本語-0f766e?style=flat-square)](./README.ja.md)
[![English](https://img.shields.io/badge/Language-English-2563eb?style=flat-square)](./README.md)
[![简体中文](https://img.shields.io/badge/%E8%AF%AD%E8%A8%80-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-dc2626?style=flat-square)](./README.zh-CN.md)
[![Español](https://img.shields.io/badge/Idioma-Espa%C3%B1ol-f59e0b?style=flat-square)](./README.es.md)
[![Português (BR)](https://img.shields.io/badge/Idioma-Portugu%C3%AAs%20(BR)-16a34a?style=flat-square)](./README.pt-BR.md)
[![한국어](https://img.shields.io/badge/%EC%96%B8%EC%96%B4-%ED%95%9C%EA%B5%AD%EC%96%B4-7c3aed?style=flat-square)](./README.ko.md)
[![Français](https://img.shields.io/badge/Langue-Fran%C3%A7ais-0891b2?style=flat-square)](./README.fr.md)
[![Deutsch](https://img.shields.io/badge/Sprache-Deutsch-4b5563?style=flat-square)](./README.de.md)
[![Русский](https://img.shields.io/badge/%D0%AF%D0%B7%D1%8B%D0%BA-%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-b91c1c?style=flat-square)](./README.ru.md)
[![Tiếng Việt](https://img.shields.io/badge/Ng%C3%B4n%20ng%E1%BB%AF-Ti%E1%BA%BFng%20Vi%E1%BB%87t-ea580c?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux는 명세 주도 개발에 기반한 AI 코딩 에이전트의 자율 개발을 오케스트레이션하는 도구입니다. 사람이 계속 붙어 있지 않아도 변경 사항을 계속 진행시키며, 적용, 수용 판정, 아카이브, 최종 머지까지의 일련의 흐름을 수행합니다.

Conflux가 지향하는 것은 일회성 코드 생성만이 아닙니다. 먼저 명세를 정하고, 그 명세에 따라 변경을 차곡차곡 쌓아 가며, 실제 운영을 염두에 둔 일정 규모의 완성품을 지속적으로 키워 나가는 것입니다.

또한 Conflux는 특정 AI 벤더에 종속되지 않습니다. [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), [OpenCode](https://opencode.ai/) 같은 도구를 서로 바꿔 가며 사용할 수 있도록 설계되어 있습니다.

## Conflux의 기본 개념

- **자는 동안에도 진행되는 자율 개발**: 사람이 계속 지켜보지 않아도 AI 에이전트가 변경 사항을 순서대로 처리하며 개발을 앞으로 밀어 줍니다.
- **명세 주도 개발**: [OpenSpec](https://github.com/openspec/openspec)을 사용해 먼저 명세를 정하고, 그 명세에 따라 구현, 검수, 개선을 진행합니다.
- **일정 규모의 완성품을 지속적으로 성장시키기**: 일회성 생성으로 끝나지 않고, 변경을 계속 쌓아 올리면서 완성도 높은 제품에 점점 가까워집니다.

## 이를 가능하게 하는 구조

- **다중 Ralph 루프**: 반복적인 이터레이션을 통해 개선을 거듭하면서, 각 이터레이션에서 전달되는 컨텍스트를 최소화해 LLM을 효율적으로 활용합니다.
- **git worktree를 활용한 병렬 개발**: Conflux가 각 change마다 독립적인 worktree를 할당하여 여러 변경을 안전하게 병렬로 진행할 수 있습니다.
- **벤더 독립적인 에이전트 선택**: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), [OpenCode](https://opencode.ai/) 등 특정 벤더에 고정되지 않으며, 목적에 따라 구현 담당 또는 평가 담당 에이전트를 바꿔 사용할 수 있습니다.
- **구현과 수용 역할의 분리**: 구현을 밀어붙이는 역할과 결과물을 검수하는 역할을 분리함으로써, 빠른 코더와 더 똑똑한 리뷰어를 조합할 수 있습니다. 이를 통해 LLM을 더 효율적으로 쓰면서도 전체 개발 속도를 높일 수 있습니다.

요약하면 Conflux는 **명세 주도 개발에 기반한 자율 개발을, 병렬 실행과 역할 분리를 갖춘 현실적인 개발 워크플로로 운영하면서 일정 규모의 완성품을 지속적으로 전진시키기 위한 오케스트레이터**입니다.

## 주요 사용 방법

| 사용 방법 | 명령어 |
|------|---------|
| TUI | `cflx` |
| 헤드리스 실행 | `cflx run` |

서버 모드, 원격 TUI, REST API, `cflx service`에 대해서는 [서버 모드 가이드(영문)](docs/guides/SERVER.md)를 참고하세요.

## 빠른 시작

초기 설정은 [QUICKSTART.ko.md](QUICKSTART.ko.md)를 참고하세요.

## 기본 명령어

```bash
# TUI
cflx

# 헤드리스 실행
cflx run

# 특정 변경만 실행
cflx run --change add-feature-x

# 설정 파일 초기화
cflx init

# bundled skills 설치
cflx install-skills
```

## 설정

설정 파일 형식은 JSONC입니다.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

템플릿 생성:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

자세한 설정 예시, 훅, 워크스페이스 실행, 명령 큐 설명은 영어 README를 참고하세요.

## 설치

```bash
cargo install cflx
```

## 문서

| 문서 | 설명 |
|----------|-------------|
| [QUICKSTART.ko.md](QUICKSTART.ko.md) | 초기 설정 |
| [서버 모드 가이드(영문)](docs/guides/SERVER.md) | 서버 모드, 원격 TUI, Web UI, REST API, 백그라운드 서비스 |
| [README.md](README.md) | 전체 문서(영어) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | 사용 예시 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 기여 가이드 |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | 개발 가이드 |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | 릴리스 가이드 |
| [docs/openapi.yaml](docs/openapi.yaml) | API 명세 |

## 라이선스

MIT
