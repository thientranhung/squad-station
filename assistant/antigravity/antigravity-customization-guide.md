---
name: antigravity-customization-guide
description: Kiến thức về cách Antigravity xử lý Rules, Workflows, Skills — từ official docs
---

# Antigravity Customization Guide

> Nguồn: https://antigravity.google/docs — đọc ngày 2026-04-14

---

## 1. Rules

**Định nghĩa:** Constraints cho Agent — guide hành vi, tasks, stack, style.

- File Markdown — giới hạn **12,000 characters** mỗi file
- **Global Rules**: `~/.gemini/GEMINI.md` → áp dụng tất cả workspaces
- **Workspace Rules**: `.agents/rules/` (backward compat: `.agent/rules/`)

**4 chế độ activation cho Workspace Rules:**

| Chế độ | Mô tả |
|--------|--------|
| Manual | Kích hoạt qua @ mention trong input box |
| Always On | Luôn áp dụng |
| Model Decision | Dựa trên description, model tự quyết |
| Glob | Áp dụng theo file pattern (VD: `.js`, `src/**/*.ts`) |

---

## 2. Workflows

**Định nghĩa:** Structured sequence of steps — guiding model qua chuỗi actions.

- File Markdown — giới hạn **12,000 characters** mỗi file
- Chứa: Title, Description, Steps (chuỗi bước cụ thể)
- **Invoke**: Gõ `/workflow-name` trong agent chat
- **Gọi lồng**: Một workflow có thể gọi workflow khác (VD: "Call /workflow-2")

**Cách tạo:**
1. Customizations panel → "..." dropdown → agent panel
2. Navigate to Workflows panel
3. Click **+ Global** (tất cả workspaces) hoặc **+ Workspace** (chỉ workspace hiện tại)

**Vị trí lưu:**
- Workspace: `.agents/workflows/` (backward compat: `.agent/workflows/`)
- Global: managed by Antigravity UI

**Agent-Generated Workflows:** Agent có thể tự tạo workflows dựa trên conversation history.

---

## 3. Skills

**Định nghĩa:** Reusable packages of knowledge mở rộng khả năng agent (open standard).

Mỗi skill chứa:
- **Instructions** để approach task cụ thể
- **Best practices** và conventions
- **Optional scripts and resources**

**Cách Agent dùng:** Khi bắt đầu conversation, agent thấy danh sách skills + descriptions. Nếu relevant → agent tự đọc full SKILL.md và follow.

**Vị trí lưu:**

| Location | Scope |
|----------|-------|
| `<workspace>/.agents/skills/<skill-folder>/` | Workspace-specific |
| `~/.gemini/antigravity/skills/<skill-folder>/` | Global |

**Skill folder structure:**
- `SKILL.md` (required) — YAML frontmatter (name, description) + instructions
- Optional: `scripts/`, `examples/`, `resources/`

**Note:** Antigravity defaults to `.agents/` nhưng backward compat `.agent/`

---

## 4. So sánh nhanh

| | Rules | Workflows | Skills |
|---|-------|-----------|--------|
| **Mục đích** | Constraints, guide hành vi | Step-by-step trajectory | Reusable knowledge |
| **Kích hoạt** | Auto/Manual/Glob/Model | `/slash-command` | Agent tự match |
| **Giới hạn** | 12,000 chars | 12,000 chars | Không rõ |
| **Gọi lồng** | Không | Có | Không |

---

## 5. Áp dụng vào squad-station

### Cấu trúc hiện tại
```
.agent/
├── commands/          ← (trống)
├── skills/            ← 4 OpenSpec skills (SKILL.md chuẩn)
└── workflows/         ← 4 workflows → /opsx-* slash commands

assistant/antigravity/
├── assistant-squad-station.md   ← @ mention reference
├── assistant-chat-agent.md      ← @ mention reference
├── handoff-with-obsidian.md     ← Tài liệu tĩnh (chưa là slash command)
└── onboard-with-obsidian.md     ← Tài liệu tĩnh (chưa là slash command)
```

### Nhận xét cần lưu ý
1. **Workflow ↔ Skill trùng ~95%** — 12,000 chars limit → nên tách rõ vai trò
2. **handoff/onboard** — nếu muốn `/slash-command`, phải nằm trong `.agents/workflows/`
3. **assistant prompts** — có thể chuyển thành Rules (Manual activation) để @ mention chính thức
4. **`.agent/commands/`** — chưa tận dụng, có thể dùng cho helper scripts
