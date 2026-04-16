# Hướng dẫn Thiết lập và Sử dụng Squad Station: Trạm Điều Phối AI Agents

*(Bài viết dành cho Developers muốn thiết lập một hệ sinh thái AI Agents tự động hóa quy trình làm việc phần mềm)*

---

## 1. Tại sao tôi lại tạo ra Squad Station? (Tư tưởng & Triết lý)

Khi làm việc với các công cụ AI CLI như Claude Code, Gemini CLI hay các dòng AI Agents khác, chúng ta thường gặp phải một điểm nghẽn: **một agent duy nhất phải gánh vác mọi vai trò** (từ phân tích yêu cầu, thiết kế kiến trúc, cho đến viết code và test). Điều này rất dễ dẫn đến việc tràn ngữ cảnh (context length), suy giảm sự tập trung, hoặc mất định hướng khi project phình to.

Đó là lý do tôi xây dựng **Squad Station** – một hệ thống định tuyến và điều phối (Orchestrator) dành riêng cho các "biệt đội" AI. Hệ thống này hoạt động dựa trên cơ chế phân luồng thông điệp qua **tmux** và cơ sở dữ liệu **SQLite**.

Tư tưởng cốt lõi của Squad Station xoay quanh 5 trụ cột:

1. **Orchestrator là "Bản sao" của bạn (HITL Proxy):**
   Thay vì bạn phải tự tay tạo prompt giao việc cho từng agent mỗi ngày, Orchestrator đóng vai trò là một Project Manager (PM) hay Tech Lead. Nó sẽ đọc tài liệu (playbook), theo dõi tiến độ, và thay mặt bạn chia nhỏ công việc để giao cho các agent cấp dưới. Bạn chỉ cần tương tác ở mức độ tư duy (high-level) với Orchestrator.

2. **Quy trình chuẩn hóa bằng SDD Playbooks (Tổ chức có kỷ luật):**
   Thay vì để AI làm việc tự do (free-prompting) dẫn đến việc dễ đi chệch hướng, hệ thống áp dụng các quy trình phát triển phần mềm (SDD - Structured Development Design) như BMad, Get-Shit-Done (GSD), hay OpenSpec dưới dạng các **Playbooks**. Các Playbook này đóng vai trò như "nội quy công ty", ép các agents tuân thủ chặt chẽ các bước làm việc (từ phân tích, lên kế hoạch, commit code đến review). Nhờ vậy, tập hợp các agents rời rạc biến thành một tổ chức (squad) làm việc đồng bộ, có quy củ.

3. **Squad Station chỉ là "Bưu điện" (Post Office):**
   Bản thân hệ thống Squad Station là một stateless Rust CLI. Nó "không có não", không tự suy diễn nội dung mà chỉ làm nhiệm vụ nhận/gửi thông điệp một cách chuẩn xác từ Orchestrator đến Worker và ngược lại. Điều này đảm bảo sự nhẹ nhàng, tính ổn định và bảo mật (không rò rỉ context).

4. **Các Worker Agents hoạt động độc lập và thụ động:**
   Các agent (như Coder, Tester) không cần biết về sự tồn tại của nhau. Chúng nhận task một cách tuần tự thông qua session tmux của mình, thực thi, và báo cáo kết quả hoàn thành thông qua các **signal hooks** được cài cắm hoàn toàn tự động.

5. **Provider-Agnostic (Không phụ thuộc nền tảng):**
   Bạn không bị khóa chặt vào một hệ sinh thái AI nào cả. Bạn có thể dùng Claude Code làm Orchestrator, Codex làm Backend, và Gemini CLI làm Frontend/QA trong cùng một dự án. Squad Station là trạm trung chuyển kết nối tất cả.

---

## 2. Hướng dẫn Cài đặt

Squad Station được thiết kế để tương tác trực tiếp với terminal của bạn.

### Yêu cầu hệ thống
- Hệ điều hành: **macOS** hoặc **Linux** (Không hỗ trợ Windows nguyên bản do yêu cầu tmux).
- **tmux**: Bắt buộc phải cài đặt (công cụ cốt lõi để quản lý session cho từng agent).
- **Node.js 14+**: Yêu cầu nếu bạn chọn cách cài qua npm.

### Các cách cài đặt

**Cách 1: Cài đặt qua npm (Khuyên dùng)**

```bash
npx squad-station@latest install
```

Cách này tự động tải native binary tương thích với hệ điều hành của bạn, cài vào `~/.squad/bin/` (tự thêm vào `~/.zshrc` / `~/.bashrc`), và tạo sẵn các thư mục mẫu:

```
.squad/
├── sdd/          # Playbook templates (GSD, BMad, OpenSpec, Superpowers)
├── rules/        # Git workflow rule templates per SDD methodology
└── examples/     # Example squad.yml configs
```

**Cách 2: Cài đặt qua curl**

```bash
curl -fsSL https://raw.githubusercontent.com/thientranhung/squad-station/master/install.sh | sh
```

**Cách 3: Build trực tiếp từ Source (Dành cho Rust Devs)**

```bash
git clone https://github.com/thientranhung/squad-station.git
cd squad-station
cargo build --release
# Binary tại: target/release/squad-station
```

---

## 3. Cấu hình và Chạy ví dụ đầu tiên (Quick Start)

### Bước 1: Tạo file `squad.yml`

Tại thư mục gốc của dự án, tạo file `squad.yml` khai báo Orchestrator và các Worker Agents:

```yaml
project: my-awesome-app

# Chọn quy trình phát triển SDD (tùy chọn)
sdd:
  - name: get-shit-done
    playbook: ".squad/sdd/gsd-playbook.md"

# (v0.9.0+) Validate sự tồn tại của playbook file khi init
sdd-playbook:
  - gsd

# Telegram notifications (tùy chọn — credentials đặt trong .env.squad)
telegram:
  enabled: true
  notify_agents: [orchestrator]   # hoặc "all"

# "Sếp" — Orchestrator
orchestrator:
  provider: claude-code
  role: orchestrator
  model: opus
  description: "Team leader, phân tích yêu cầu và điều phối task cho các agent."

# "Nhân viên" — Worker Agents
agents:
  - name: implement
    provider: claude-code
    role: worker
    model: sonnet
    description: "Senior coder, viết code và fix bug."

  - name: frontend
    provider: gemini-cli
    role: worker
    model: gemini-3.1-pro-preview
    description: "Frontend/UI developer."
```

**Các provider và model hợp lệ:**

| Provider | Models |
|----------|--------|
| `claude-code` | `opus`, `sonnet`, `haiku` |
| `gemini-cli` | `gemini-3.1-pro-preview`, `gemini-3-flash-preview` |
| `codex` | `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.3-codex`, ... |

### Bước 2: Khởi tạo Trạm (Init)

```bash
squad-station init
```

Lệnh này thực hiện hàng loạt tác vụ đằng sau hậu trường:
- Đăng ký Orchestrator và các Agents vào SQLite (`.squad/station.db`)
- Khởi tạo các **tmux session** riêng biệt cho từng agent
- Cài đặt **signal hooks** vào `.claude/settings.json` / `.gemini/settings.json` để agent tự báo cáo hoàn thành task
- Generate file playbook tại `.claude/commands/squad-orchestrator.md` (hoặc `.gemini/commands/squad-orchestrator.toml`)
- Inject bootstrap block vào `CLAUDE.md` / `GEMINI.md` để Orchestrator không mất vai trò sau `/clear`
- Copy SDD git workflow rules vào `.claude/rules/` hoặc `.gemini/rules/`
- Tự khởi động Watchdog daemon ngầm (30s interval) để theo dõi liveness của các session
- Chạy post-init health check và in kết quả ra terminal

### Bước 3: Kích hoạt Orchestrator

Attach vào tmux session của Orchestrator (tên session có dạng `<project-name>-orchestrator`), sau đó gõ:

```bash
# Cho Claude Code, Codex, hoặc Gemini CLI — đều dùng slash command:
/squad-orchestrator
```

Orchestrator sẽ đọc file playbook, nhận thức được vai trò quản lý của mình, và bắt đầu giao task cho các worker agent thông qua `squad-station send`.

### Bước 4: Giám sát và Vận hành

**Xem tổng quan dự án:**
```bash
squad-station status           # Tóm tắt project + trạng thái từng agent
squad-station agents           # Roster với live status (idle/busy/dead)
squad-station list             # Danh sách messages (--agent, --status, --limit)
squad-station peek <agent>     # Task kế tiếp của một agent cụ thể
```

**Kiểm tra sức khỏe hệ thống:**
```bash
squad-station doctor           # 6-check health: config, playbooks, tmux, DB, hooks, version
```

**Giao task thủ công (khi cần bypass Orchestrator):**
```bash
squad-station send implement --body "Fix the login bug in auth.rs"
squad-station send implement --body "Deploy to staging" --priority urgent
```

**Xử lý sự cố:**
```bash
squad-station reconcile        # Phát hiện và fix stuck agents (busy trong DB nhưng idle trong tmux)
squad-station reconcile --dry-run  # Xem trước mà không thay đổi DB
squad-station freeze           # Chặn Orchestrator giao task (user giành quyền kiểm soát)
squad-station unfreeze         # Trả quyền lại cho Orchestrator
```

**Cập nhật squad khi thay đổi `squad.yml`:**
```bash
squad-station update           # Launch agent mới, restart agent bị thay đổi, skip agent đang chạy
```

**Gỡ cài đặt và dọn dẹp:**
```bash
squad-station uninstall        # Gỡ hooks, files, sessions khỏi project này
squad-station clean            # Kill sessions + xóa DB (--all để xóa thêm logs)
```

---

## 4. Luồng hoạt động tự động (Signal Flow)

Khi một Worker Agent hoàn thành task, luồng thông báo diễn ra hoàn toàn tự động:

```
Agent hoàn thành task
    → Hook (Stop/AfterAgent) tự kích hoạt
    → squad-station signal        ← agent tự gọi, không cần can thiệp
    → DB cập nhật message → completed
    → Orchestrator nhận thông báo qua tmux send-keys
    → Orchestrator đọc kết quả và giao task tiếp theo
```

Nếu agent cần thêm thông tin giữa chừng (chưa xong task), nó có thể ping Orchestrator:

```bash
squad-station notify --body "Cần clarify về API endpoint /users trước khi tiếp tục"
```

Logs tại `.squad/log/signal.log` để debug khi cần.

---

## 5. Chẩn đoán sự cố với `doctor`

Khi hệ thống hoạt động bất thường, chạy:

```bash
squad-station doctor
```

Lệnh này kiểm tra 6 mục:
1. **Config** — `squad.yml` có hợp lệ không
2. **SDD Playbooks** — các file playbook được khai báo có tồn tại không
3. **tmux** — các session agent có đang chạy không
4. **DB** — database có truy cập được không
5. **Hooks** — hook entries có trong settings.json, binary path có còn hợp lệ không (phát hiện stale path sau upgrade)
6. **Version** — binary đang dùng có phải version mới nhất không

```bash
squad-station doctor --json    # Output machine-readable cho automation
```

---

## Lời kết

Squad Station không phải là một LLM mới hay một công cụ AI sinh code thần thánh. Nó là **một phương pháp tiếp cận mới về kiến trúc phân tán cho AI**.

Bằng cách tách bạch rõ ràng giữa "Tư duy quản lý" (Orchestrator) và "Năng lực thực thi" (Workers), cộng với việc ứng dụng các quy trình phát triển chuyên nghiệp (SDD playbooks), bạn hoàn toàn có thể xây dựng các dự án phức tạp một cách bền bỉ, an toàn, không sợ context bùng nổ, mà vẫn giữ được chuẩn mực coding của team.

Hãy cài đặt `squad-station` ngay hôm nay, thử thiết lập một quy trình Get-Shit-Done cơ bản, và trao quyền tự chủ cho "Biệt đội AI" của riêng bạn!
