# Conflux

[![日本語](https://img.shields.io/badge/%E6%97%A5%E6%9C%AC%E8%AA%9E-informational?style=flat-square)](./README.ja.md) [![English](https://img.shields.io/badge/English-informational?style=flat-square)](./README.md) [![简体中文](https://img.shields.io/badge/%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-informational?style=flat-square)](./README.zh-CN.md) [![Español](https://img.shields.io/badge/Espa%C3%B1ol-informational?style=flat-square)](./README.es.md) [![Português (BR)](https://img.shields.io/badge/Portugu%C3%AAs%20(BR)-informational?style=flat-square)](./README.pt-BR.md) [![한국어](https://img.shields.io/badge/%ED%95%9C%EA%B5%AD%EC%96%B4-informational?style=flat-square)](./README.ko.md) [![Français](https://img.shields.io/badge/Fran%C3%A7ais-informational?style=flat-square)](./README.fr.md) [![Deutsch](https://img.shields.io/badge/Deutsch-informational?style=flat-square)](./README.de.md) [![Русский](https://img.shields.io/badge/%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-informational?style=flat-square)](./README.ru.md) [![Tiếng Việt](https://img.shields.io/badge/Ti%E1%BA%BFng%20Vi%E1%BB%87t-informational?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux là công cụ điều phối quá trình phát triển tự vận hành của các tác nhân lập trình AI dựa trên phương pháp phát triển theo đặc tả. Ngay cả khi không có con người theo dõi liên tục, nó vẫn tiếp tục luân chuyển các thay đổi, thực hiện áp dụng, đánh giá chấp nhận, lưu trữ và cuối cùng là hợp nhất trong một luồng xuyên suốt.

Mục tiêu của Conflux không phải là tạo mã một lần rồi dừng lại. Trước hết, đặc tả được xác định rõ ràng; sau đó, các thay đổi được tích lũy theo đúng đặc tả đó để liên tục nuôi dưỡng một sản phẩm có quy mô nhất định, hướng tới vận hành thực tế.

Conflux cũng không phụ thuộc vào một nhà cung cấp AI cụ thể. Nó được thiết kế để có thể thay thế linh hoạt giữa các công cụ như [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/) và [OpenCode](https://opencode.ai/).

## Các khái niệm cốt lõi của Conflux

- **Phát triển tự vận hành tiếp tục tiến lên ngay cả khi bạn đang ngủ**: ngay cả khi không có người giám sát thường trực, các tác nhân AI vẫn xử lý lần lượt các thay đổi và đưa việc phát triển tiến về phía trước.
- **Phát triển theo đặc tả**: sử dụng [OpenSpec](https://github.com/openspec/openspec) để xác định đặc tả trước, rồi triển khai, nghiệm thu và cải tiến dựa trên đặc tả đó.
- **Liên tục nuôi dưỡng một sản phẩm có quy mô nhất định**: thay vì kết thúc ở một lần sinh mã, các thay đổi được chồng dần lên nhau để ngày càng tiến gần hơn tới một sản phẩm hoàn chỉnh.

## Cơ chế giúp hiện thực hóa điều đó

- **Nhiều vòng lặp Ralph lồng nhau**: hệ thống cải thiện qua các vòng lặp lặp đi lặp lại, đồng thời giữ cho lượng ngữ cảnh được truyền ở mỗi vòng là tối thiểu để sử dụng LLM hiệu quả hơn.
- **Phát triển song song với `git worktree`**: Conflux gán một worktree độc lập cho mỗi change, nhờ đó nhiều thay đổi có thể được triển khai song song một cách an toàn.
- **Lựa chọn tác nhân không phụ thuộc nhà cung cấp**: không bị cố định vào một vendor cụ thể, bạn có thể thay thế tác nhân triển khai hoặc tác nhân đánh giá tùy theo mục đích bằng [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), [OpenCode](https://opencode.ai/) và các công cụ khác.
- **Tách biệt vai trò triển khai và nghiệm thu**: bằng cách tách vai trò thúc đẩy triển khai khỏi vai trò nghiệm thu kết quả, bạn có thể kết hợp những coder nhanh với những reviewer sắc bén hơn. Nhờ đó, LLM được sử dụng hiệu quả hơn, đồng thời tốc độ phát triển tổng thể cũng được nâng cao.

Tóm lại, Conflux là **một bộ điều phối giúp liên tục thúc đẩy một sản phẩm có quy mô nhất định bằng cách vận hành phát triển tự động dựa trên đặc tả dưới dạng một quy trình phát triển thực tế, có thực thi song song và tách biệt vai trò**.

## Cách sử dụng chính

| Cách sử dụng | Lệnh |
|------|---------|
| TUI | `cflx` |
| Chạy headless | `cflx run` |

Đối với chế độ máy chủ, TUI từ xa, REST API và `cflx service`, hãy xem [hướng dẫn chế độ máy chủ (bản tiếng Anh)](docs/guides/SERVER.md).

## Bắt đầu nhanh

Để thiết lập lần đầu, hãy xem [QUICKSTART.vi.md](QUICKSTART.vi.md).

## Các lệnh cơ bản

```bash
# TUI
cflx

# Chạy headless
cflx run

# Chỉ chạy một thay đổi cụ thể
cflx run --change add-feature-x

# Khởi tạo tệp cấu hình
cflx init

# Cài đặt bundled skills
cflx install-skills
```

## Cấu hình

Tệp cấu hình sử dụng định dạng JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Tạo mẫu:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

Để xem các ví dụ cấu hình chi tiết hơn, cùng giải thích về hooks, chạy theo workspace và hàng đợi lệnh, hãy tham khảo README tiếng Anh.

## Cài đặt

```bash
cargo install cflx
```

## Tài liệu

| Tài liệu | Mô tả |
|----------|-------------|
| [QUICKSTART.vi.md](QUICKSTART.vi.md) | Thiết lập lần đầu |
| [Hướng dẫn chế độ máy chủ (bản tiếng Anh)](docs/guides/SERVER.md) | Chế độ máy chủ, TUI từ xa, Web UI, REST API, dịch vụ nền |
| [README.md](README.md) | Tài liệu đầy đủ (tiếng Anh) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Ví dụ sử dụng |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Hướng dẫn đóng góp |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Hướng dẫn phát triển |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Hướng dẫn phát hành |
| [docs/openapi.yaml](docs/openapi.yaml) | Đặc tả API |

## Giấy phép

MIT
