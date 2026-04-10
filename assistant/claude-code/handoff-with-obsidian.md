---
description: Lưu context session hiện tại vào Obsidian vault dưới dạng handoff file để phục hồi sau
---

# Handoff with Obsidian

Tạo file handoff trong Obsidian vault của dự án hiện tại để lưu toàn bộ context cho session sau phục hồi.

## Quy trình bắt buộc

### Bước 1 — Xác định Obsidian project folder

**Obsidian base path (CỐ ĐỊNH):**
```
/Users/tranthien/Library/Mobile Documents/iCloud~md~obsidian/Documents/Obsidian/1-Projects
```

**Logic tự động detect project folder:**

1. Lấy tên repo/dir hiện tại:
   ```bash
   basename "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
   ```

2. Liệt kê tất cả folders trong base path:
   ```bash
   ls "/Users/tranthien/Library/Mobile Documents/iCloud~md~obsidian/Documents/Obsidian/1-Projects"
   ```

3. **Fuzzy match** — tìm folder có độ tương đồng cao nhất với tên repo:
   - Exact match (case-insensitive) nếu có
   - Nếu không, chọn folder có overlap nhiều nhất về mặt ngữ nghĩa/ký tự với tên repo
   - Dùng judgment linh hoạt, không cần thuật toán cứng

4. **Xử lý kết quả:**
   - **1 match duy nhất** → dùng luôn, thông báo cho user
   - **Không có match** → hỏi user: "Không tìm thấy Obsidian folder match với repo '<tên>'. Chọn 1 trong các folder hiện có hoặc tạo mới:" (show list)
   - **Nhiều match** → hỏi user chọn 1 trong các candidates

### Bước 2 — Hỏi user về topic của handoff

Hỏi ngắn gọn bằng tiếng Việt:
> "Topic ngắn gọn của handoff này là gì? (sẽ dùng trong tên file, VD: ASI-bugs, refactor-auth, feature-login)"

Chờ user trả lời trước khi tiếp tục.

### Bước 3 — Generate filename

Format: `HANDOFF-YYYY-MM-DD-<topic>.md`

Lấy date:
```bash
date +%Y-%m-%d
```

### Bước 4 — Tổng hợp và ghi nội dung handoff

- **Review toàn bộ conversation** hiện tại
- Fill template bên dưới **BẰNG TIẾNG VIỆT**
- Phải đủ chi tiết để session khác đọc xong là hiểu và phục hồi được context hoàn toàn
- Không được bỏ qua section nào — nếu không có nội dung, ghi `N/A` hoặc "Không có"

### Bước 5 — Xác nhận

Sau khi ghi file thành công:
- Thông báo path đầy đủ của file đã tạo
- Tóm tắt ngắn gọn những gì đã lưu
- Nhắc user cách resume sau này: `/onboard-with-obsidian`

---

## Template BẮT BUỘC (tiếng Việt)

```markdown
# HANDOFF — <topic>

**Date:** YYYY-MM-DD
**Session paused for:** <lý do tạm dừng — ngắn gọn>
**Resume from:** This document

---

## 🎯 Original User Request

<Mô tả đầy đủ yêu cầu gốc của user ngay từ đầu session. Gồm TẤT CẢ các task liên quan, kể cả task chưa bắt đầu. Ghi rõ trạng thái từng task (Done / In Progress / Pending).>

---

## 🔍 Investigation Findings

<Tất cả phát hiện quan trọng từ investigation:
- Root cause analysis
- Evidence (từ production DB, logs, screenshots, v.v.)
- Discovery phụ (sibling bugs, related issues)
- Quyết định kỹ thuật đã thống nhất

Viết đủ chi tiết để không phải investigate lại.>

---

## ✅ Implementation Done

<Liệt kê chi tiết những gì đã hoàn thành. Nếu có nhiều branches/PRs, dùng bảng:

| Phase | Branch | PR | Tests | Status |
|-------|--------|-----|-------|--------|
| ... | ... | ... | ... | ... |

Kèm mô tả ngắn về từng thay đổi chính.>

---

## ⏳ Pending / In Progress

<Những task đang dở, đang chờ, hoặc chưa bắt đầu. Ghi rõ:
- Task nào đang ở bước nào
- Cần làm gì tiếp
- Ai/cái gì đang block nó>

---

## 🚫 Known Issues / Blockers

<Lý do pause session, bugs chưa fix, limitation của hạ tầng, workarounds đang dùng. Quan trọng để session sau biết tránh/xử lý.>

---

## 📋 Resume Checklist

<Các bước cụ thể cần làm khi quay lại, đánh số rõ ràng:

### Step 1: <tên bước>
\`\`\`bash
<command cụ thể nếu có>
\`\`\`
<Giải thích>

### Step 2: ...

Viết đủ chi tiết để chỉ cần follow là tiếp tục được. Ghi rõ cả các quyết định đang pending cần hỏi user.>

---

## 📂 Key Files / Branches / PRs

<Liệt kê:
- Files đã sửa (đường dẫn đầy đủ)
- Branches đang tồn tại liên quan
- PRs đang open
- Commits quan trọng>

---

## 🔗 Key References

<Links, credentials, tmux sessions, external resources, documentation cần biết.
VD:
- Production access: tmux session `<name>`
- Test credentials: ...
- Repo rules: ...
- Related docs: ...>

---

## 📝 Session Metadata

- **User:** <tên>
- **Language:** <ngôn ngữ user dùng>
- **Assistant role:** <vai trò đang đóng>
- **Date:** YYYY-MM-DD
- **Current working directory:** <pwd>
- **Current git branch:** <branch hiện tại>
- **Last action before pause:** <hành động cuối cùng>
```

---

## Quy tắc nghiêm ngặt

1. **LUÔN viết bằng tiếng Việt** trong file handoff
2. **KHÔNG bỏ qua section nào** trong template — thiếu section = handoff không đủ thông tin
3. **Phải review toàn bộ conversation** trước khi ghi, không chỉ vài message cuối
4. **Xác nhận với user** nếu có điểm mơ hồ (folder Obsidian, topic name, v.v.)
5. **Không commit file này vào git** — đây là file trong Obsidian vault, không thuộc về repo
6. **Ghi theo template cố định** — không tự ý thêm/bớt section
7. **Nội dung phải đủ chi tiết** để assistant/session khác đọc xong là hiểu ngay, không cần hỏi lại user
