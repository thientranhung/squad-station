# Squad Station — Báo cáo lỗi

**Kiểm thử:** 2026-03-15
**Binary:** squad-station v0.2.0 (target/release/squad-station)
**Dự án test:** squad-station-landing-page (tmux session: squad-station-testing)

---

## BUG-01: Signal hoàn thành sai task (lệch LIFO vs FIFO) [NGHIÊM TRỌNG]

**Mô tả:** `signal` hoàn thành task **mới nhất** (`ORDER BY created_at DESC`), nhưng `peek` trả về task **cũ nhất** (`ORDER BY created_at ASC` theo priority). Khi agent có nhiều task, nó peek task A, làm việc trên đó, signal, nhưng task B (mới hơn) lại bị đánh dấu hoàn thành.

**Tái hiện:**
```bash
squad-station send squad-station-implement --body "task A"
squad-station send squad-station-implement --body "task B"
squad-station peek squad-station-implement   # Trả về "task A" (cũ nhất)
squad-station signal squad-station-implement  # Hoàn thành "task B" (mới nhất!)
```

**Vị trí:** `src/db/messages.rs:65` — `ORDER BY created_at DESC LIMIT 1` nên đổi thành `ASC`.

---

## BUG-02: Signal đặt agent về idle dù còn task chưa xử lý [CAO]

**Mô tả:** Sau khi `signal` hoàn thành một task, trạng thái agent bị đặt vô điều kiện về `idle`, ngay cả khi vẫn còn task `processing` trong hàng đợi. Lệnh `status` hiển thị agent là "idle" với "N pending" — mâu thuẫn.

**Tái hiện:**
```bash
squad-station send squad-station-implement --body "task 1"
squad-station send squad-station-implement --body "task 2"
squad-station signal squad-station-implement
squad-station status  # Hiển thị: implement idle | 1 pending
```

**Kỳ vọng:** Agent nên giữ trạng thái `busy` nếu vẫn còn task đang xử lý.

**Vị trí:** `src/commands/signal.rs:102-108` — cần kiểm tra số message processing còn lại trước khi chuyển sang idle.

---

## BUG-03: Tên agent yêu cầu prefix đầy đủ của project — không hỗ trợ tên rút gọn [TRUNG BÌNH]

**Mô tả:** Agent được khai báo là `implement` trong `squad.yml` nhưng được lưu trong DB là `squad-station-implement`. Tất cả lệnh yêu cầu tên có prefix đầy đủ. Tên rút gọn sẽ báo lỗi "Agent not found".

**Tái hiện:**
```bash
squad-station send implement --body "test"       # Lỗi: Agent not found: implement
squad-station list --agent implement              # Không tìm thấy message (lỗi im lặng)
squad-station peek implement                      # Không có task chờ (lỗi im lặng)
```

**Kỳ vọng:** Chấp nhận cả `implement` và `squad-station-implement`. Tối thiểu, gợi ý tên đầy đủ khi tên rút gọn thất bại.

---

## BUG-04: `signal` cho agent không tồn tại vẫn thành công im lặng [TRUNG BÌNH]

**Mô tả:** `squad-station signal nonexistent-agent` không có output nào và exit 0. Theo thiết kế dành cho ngữ cảnh hook (HOOK-03), nhưng gây nhầm lẫn khi dùng CLI thủ công.

**Tái hiện:**
```bash
squad-station signal nonexistent  # Không output, exit 0
```

**Ghi chú:** Đây là hành vi có chủ đích cho hooks nhưng nên in ra message khi chạy tương tác (phát hiện TTY).

**Vị trí:** `src/commands/signal.rs:42-44` — `return Ok(())` im lặng khi không tìm thấy agent.

---

## BUG-05: `peek` không kiểm tra agent tồn tại [THẤP]

**Mô tả:** `peek nonexistent-agent` hiển thị "No pending tasks for nonexistent" thay vì "Agent not found". Gây hiểu lầm — người dùng nghĩ agent tồn tại nhưng không có task.

**Tái hiện:**
```bash
squad-station peek nonexistent  # "No pending tasks for nonexistent"
```

**Kỳ vọng:** "Agent not found: nonexistent"

---

## BUG-06: `from_agent` bị hardcode thành "orchestrator" (không có prefix) [THẤP]

**Mô tả:** Lệnh `send` hardcode `from_agent` là `"orchestrator"` (dòng 40 của send.rs), trong khi agent orchestrator được đăng ký là `squad-station-orchestrator`. Cột FROM trong `list` hiển thị "orchestrator" trong khi TO hiển thị tên có prefix đầy đủ.

**Vị trí:** `src/commands/send.rs:40` — chuỗi `"orchestrator"` bị hardcode.

---

## BUG-07: Có thể gửi task cho agent có role orchestrator [THẤP]

**Mô tả:** Không có guard ngăn `squad-station send squad-station-orchestrator --body "test"`. Orchestrator là coordinator, không phải task receiver.

**Tái hiện:**
```bash
squad-station send squad-station-orchestrator --body "some task"  # Thành công
```

**Kỳ vọng:** "Cannot send tasks to orchestrator-role agents" hoặc guard tương tự.

---

## BUG-08: Có thể gửi task với body rỗng [THẤP]

**Mô tả:** `squad-station send agent --body ""` thành công và tạo task với nội dung rỗng. Không có validation cho nội dung body.

**Tái hiện:**
```bash
squad-station send squad-station-implement --body ""  # Thành công với task rỗng
```

---

## BUG-09: Help text của `list --status` ghi "pending" nhưng status đó không tồn tại [THẤP]

**Mô tả:** `list --help` ghi `--status` chấp nhận `(pending, completed)`, nhưng message chuyển thẳng sang status `processing` (không bao giờ `pending`). `--status processing` hoạt động nhưng không được ghi trong tài liệu.

**Tái hiện:**
```bash
squad-station list --status pending     # Luôn "No messages found"
squad-station list --status processing  # Hoạt động (không ghi tài liệu)
```

**Vị trí:** `src/cli.rs` — help text cho option `--status`.

---

## BUG-10: `status` hiển thị số "pending" nhưng status thực tế là "processing" [THẤP]

**Mô tả:** Lệnh `status` hiển thị "N pending" cho mỗi agent, nhưng các message bên dưới có status `processing`, không phải `pending`. Thuật ngữ không khớp giữa hiển thị và data model.

---

## BUG-11: `init` báo "0 agent(s)" khi session đã tồn tại [THẤP]

**Mô tả:** Chạy `init` khi tmux session đã tồn tại hiển thị "Initialized squad with 0 agent(s)" dù có 3 agent đã được đăng ký. Số đếm chỉ phản ánh session mới được tạo.

**Tái hiện:**
```bash
squad-station init    # Lần đầu: "3 agent(s)"
squad-station init    # Lần thứ hai: "0 agent(s)" — gây hiểu lầm
```

---

## BUG-12: Lỗi thiếu squad.yml không thân thiện với người dùng [THẤP]

**Mô tả:** Chạy bất kỳ lệnh nào mà không có `squad.yml` trong CWD hiển thị lỗi OS thô: "No such file or directory (os error 2)". Nên hiển thị "squad.yml not found in current directory".

---

## BUG-13: Format timestamp `status_updated_at` không nhất quán trong JSON [THẤP]

**Mô tả:** Trong output `status --json`, `status_updated_at` dùng format khác nhau: RFC3339 với microseconds (`2026-03-15T11:10:37.402493+00:00`) cho agent được cập nhật qua lệnh, nhưng `2026-03-15 11:10:08` (không timezone, không dấu T) cho agent được set lúc `init`.

---

## BUG-14: Message `notify` không được lưu vào database [GHI CHÚ]

**Mô tả:** `notify` gửi tmux message đến orchestrator nhưng không tạo record trong bảng messages. Notification là tạm thời và không được theo dõi. Có thể là theo thiết kế, nhưng đáng lưu ý cho mục đích audit trail.

---

## BUG-15: `register` không thêm prefix project vào tên agent [GHI CHÚ]

**Mô tả:** `init` đăng ký agent với prefix `{project}-{name}`, nhưng `register` dùng tên thô. Quy ước đặt tên không nhất quán.

**Tái hiện:**
```bash
squad-station register test-agent --role worker  # Lưu là "test-agent"
squad-station agents  # Hiển thị "test-agent" cùng với "squad-station-implement"
```
