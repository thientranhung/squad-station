---
description: Lưu context session hiện tại vào Obsidian vault dưới dạng handoff file để phục hồi sau
---

# Handoff with Obsidian

Lưu toàn bộ context hiện tại dành cho session làm việc sau (hoặc một Agent khác) bằng cách tạo file handoff định dạng Markdown trong thư mục Obsidian của dự án. 

## Workflow Steps

**Bước 1: Xác định Obsidian project folder**
- Base path cố định: `/Users/tranthien/Library/Mobile Documents/iCloud~md~obsidian/Documents/Obsidian/1-Projects`
- Tự động detect project folder: Lấy thư mục gốc hiện tại (`basename "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"`).
- Liệt kê các thư mục trong base path. Thực hiện fuzzy match để ra folder hợp lý nhất.
- Nếu không có, hỏi người dùng. Nếu có nhiều match, yêu cầu người dùng chọn. Nếu có duy nhất 1 match, dùng ngay và thông báo.

**Bước 2: Hỏi về topic của handoff**
- Hỏi bằng tiếng Việt: "Topic ngắn gọn của handoff này là gì? (sẽ dùng trong tên file, VD: ASI-bugs, refactor-auth, feature-login)"
- **Dừng lại (Pause)** và chờ người dùng trả lời.

**Bước 3: Generate File Name**
- Format tên file: `HANDOFF-YYYY-MM-DD-<topic>.md` (dùng ngày tháng năm hiện tại).

**Bước 4: Kiểm tra và tổng hợp nội dung**
- Review toàn bộ conversation history hiện hành.
- Viết file bằng Tiếng Việt theo Template quy định bên dưới. KHÔNG BỎ QUA bất kì Section nào. Điền `N/A` nếu chưa có thông tin.

**Bước 5: Xác nhận hoàn tất**
- Thông báo path đầy đủ cho người dùng.
- Tóm tắt lưu trữ những gì.
- Nhắc người dùng dùng lệnh `/onboard-with-obsidian` khi muốn gọi lại context này.

---

## Template BẮT BUỘC để generate file

```markdown
# HANDOFF — <topic>

**Date:** YYYY-MM-DD
**Session paused for:** <lý do tạm dừng — ngắn gọn>
**Resume from:** This document

---

## 🎯 Original User Request
<Mô tả đầy đủ yêu cầu gốc của user. Những task đang tham gia. Trạng thái task (Done / In progress / Pending)>

---

## 🔍 Investigation Findings
<Thêm chứng cứ, root cause analysis, related bugs>

---

## ✅ Implementation Done
<Liệt kê tính năng đã xong, branch liên quan, PR liên quan>

---

## ⏳ Pending / In Progress
<Nhiệm vụ đang dở, thứ block tiến độ, ai là người block>

---

## 🚫 Known Issues / Blockers
<Ghi chép bug còn sót, hệ thống giới hạn>

---

## 📋 Resume Checklist
<Các bước cần làm tiếp khi quay lại Session bằng các code bash block tương ứng>

### Step 1: ...
### Step 2: ...

---

## 📂 Key Files / Branches / PRs
<Liệt kê files, config thay đổi>

---

## 🔗 Key References
<Liệt kê links, test credentials, production access>

---

## 📝 Session Metadata
- **User:** <tên>
- **Language:** Tiếng Việt
- **Date:** YYYY-MM-DD
- **Current git branch:** <branch>
- **Last action:** <hành động cuối cùng>
```
