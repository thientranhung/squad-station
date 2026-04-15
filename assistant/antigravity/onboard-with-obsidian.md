---
description: Đọc handoff file từ Obsidian vault và phục hồi context làm việc
---

# Onboard with Obsidian

Trợ lý ảo thực hiện quy trình này để phục hồi context làm việc từ handoff file đã lưu trong Obsidian vault bằng lệnh `/handoff-with-obsidian`.

## Workflow Steps

**Bước 1: Xác định Obsidian project folder**
- Truy cập base path: `/Users/tranthien/Library/Mobile Documents/iCloud~md~obsidian/Documents/Obsidian/1-Projects`
- Lấy project folder dựa vào root dir hiện hành thông qua fuzzy match tên của folder với `basename "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"`.
- Nếu có 1 folder trùng khớp, chọn ngay. Nếu không thấy, listing và hỏi user. 

**Bước 2: Tìm nội dung Handoff files**
- Quét trong folder dự án theo format: `HANDOFF-*.md`.
- Sắp xếp và in ra log tối đa 10 Handoff file mới nhất. Format: `1. 2026-04-10 — ASI-bugs`

**Bước 3: Yêu cầu chọn file**
- Nếu User truyền ngay argument cùng lệnh (VD `/onboard-with-obsidian ASI-bugs`), thì chọn tự động file trùng khớp nhất thông qua Argument đó.
- Nếu User không cung cấp, yêu cầu User nhập số thứ tự.
- **Có duy nhất 1 file:** Tự chọn và confirm với user.

**Bước 4: Phục hồi và So khớp hệ thống**
- Agent đọc toàn bộ file `HANDOFF` vừa lấy được. Nắm chắc nhiệm vụ gốc, nhiệm vụ done, nhiệm vụ pending và blockers.
- Agent tự động kiểm tra xem current state của code qua Terminal (chạy lệnh git status, gh pr list, current git branch, kiểm tra file tồn tại).
- Check xem branch thực tế và file log có lệch nhau không.

**Bước 5: Report lại thông tin tổng hợp để xác nhận**
- Báo cáo cho User bằng Tiếng Việt. Format tiêu chuẩn:
  * Nhiệm vụ gốc là gì?
  * Đã hoàn thành được gì? 
  * Cần làm gì tiếp theo (Resume Checklist)?
  * State có đồng bộ / bị lệch so với log khi Check ở B4 hay không?

**Bước 6: Xác nhận hoạt động tiếp (User Confirmation)**
- Dừng lại, hỏi User sẽ muốn đi tiếp Step nào trong Resume Checklist, trước khi tự ý thực thi các lệnh bash.