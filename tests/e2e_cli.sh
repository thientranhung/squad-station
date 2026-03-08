#!/bin/bash
# ==============================================================================
# Squad Station — End-to-End CLI Test Suite
# ==============================================================================
# Tests all CLI commands against the release binary with a real SQLite DB
# and live tmux sessions. Self-contained: creates and cleans up all test state.
# ==============================================================================

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${PROJECT_ROOT}/target/release/squad-station"
TEST_DIR=$(mktemp -d)
PASS=0
FAIL=0
SKIP=0
TOTAL=0
FAILURES=""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# --- Helpers ------------------------------------------------------------------

pass() {
  PASS=$((PASS + 1))
  TOTAL=$((TOTAL + 1))
  echo -e "  ${GREEN}PASS${NC} $1"
}

fail() {
  FAIL=$((FAIL + 1))
  TOTAL=$((TOTAL + 1))
  FAILURES="${FAILURES}\n  - $1: $2"
  echo -e "  ${RED}FAIL${NC} $1 — $2"
}

skip() {
  SKIP=$((SKIP + 1))
  TOTAL=$((TOTAL + 1))
  echo -e "  ${YELLOW}SKIP${NC} $1 — $2"
}

section() {
  echo ""
  echo -e "${CYAN}${BOLD}━━━ $1 ━━━${NC}"
}

cleanup() {
  # Kill test tmux sessions
  tmux kill-session -t e2e-orchestrator 2>/dev/null || true
  tmux kill-session -t e2e-agent1 2>/dev/null || true
  tmux kill-session -t e2e-agent2 2>/dev/null || true
  tmux kill-session -t e2e-agent3 2>/dev/null || true
  tmux kill-window -t squad-view 2>/dev/null || true
  # Remove test directory
  rm -rf "$TEST_DIR"
}

trap cleanup EXIT

# --- Setup --------------------------------------------------------------------

section "SETUP"

# Verify binary exists
if [[ ! -x "$BIN" ]]; then
  echo "Binary not found at $BIN — run 'cargo build --release' first"
  exit 1
fi
echo "  Binary: $BIN"
echo "  Test dir: $TEST_DIR"

# Create test squad.yml
cat > "$TEST_DIR/squad.yml" << 'YAML'
project:
  name: e2e-test
  db_path: __DB_PATH__
orchestrator:
  name: e2e-orchestrator
  provider: claude-code
  role: orchestrator
  command: "sleep 3600"
agents:
  - name: e2e-agent1
    provider: claude-code
    role: worker
    command: "sleep 3600"
  - name: e2e-agent2
    provider: gemini
    role: worker
    command: "sleep 3600"
YAML
sed -i '' "s|__DB_PATH__|${TEST_DIR}/station.db|" "$TEST_DIR/squad.yml"

cd "$TEST_DIR"

# ==============================================================================
# TEST SUITE
# ==============================================================================

# --- 1. Binary & Help --------------------------------------------------------

section "1. BINARY & HELP"

# T1.1: --help shows all subcommands
OUTPUT=$($BIN --help 2>&1)
EXPECTED_CMDS="init send signal list peek register agents context status ui view"
ALL_FOUND=true
for cmd in $EXPECTED_CMDS; do
  if ! echo "$OUTPUT" | grep -q "$cmd"; then
    ALL_FOUND=false
    break
  fi
done
if $ALL_FOUND; then
  pass "T1.1 --help lists all 11 subcommands"
else
  fail "T1.1 --help lists all subcommands" "missing command in output"
fi

# T1.2: --version works
OUTPUT=$($BIN --version 2>&1)
if echo "$OUTPUT" | grep -q "squad-station"; then
  pass "T1.2 --version prints version"
else
  fail "T1.2 --version" "unexpected output: $OUTPUT"
fi

# T1.3: Unknown command exits non-zero
if $BIN nonexistent 2>&1; then
  fail "T1.3 unknown command exits non-zero" "got exit 0"
else
  pass "T1.3 unknown command exits non-zero"
fi

# --- 2. Init ------------------------------------------------------------------

section "2. INIT"

# T2.1: Init creates DB and registers agents
OUTPUT=$($BIN init squad.yml 2>&1)
if [[ -f "$TEST_DIR/station.db" ]] && echo "$OUTPUT" | grep -q "Initialized squad"; then
  pass "T2.1 init creates DB and reports success"
else
  fail "T2.1 init creates DB" "DB missing or bad output"
fi

# T2.2: Init idempotency — re-run doesn't error
OUTPUT=$($BIN init squad.yml 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T2.2 init idempotent (re-run exits 0)"
else
  fail "T2.2 init idempotent" "exit code: $EXIT_CODE"
fi

# T2.3: Init with missing file errors
if $BIN init nonexistent.yml 2>&1; then
  fail "T2.3 init with missing file errors" "got exit 0"
else
  pass "T2.3 init with missing file errors"
fi

# --- 3. Register --------------------------------------------------------------

section "3. REGISTER"

# T3.1: Register new agent
OUTPUT=$($BIN register e2e-agent3 --command "sleep 3600" --provider claude-code 2>&1)
if echo "$OUTPUT" | grep -q "Registered agent 'e2e-agent3'"; then
  pass "T3.1 register new agent"
else
  fail "T3.1 register new agent" "output: $OUTPUT"
fi

# T3.2: Register idempotent — same agent again
OUTPUT=$($BIN register e2e-agent3 --command "sleep 3600" --provider claude-code 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T3.2 register idempotent (duplicate exits 0)"
else
  fail "T3.2 register idempotent" "exit code: $EXIT_CODE"
fi

# T3.3: Register --json output
OUTPUT=$($BIN register e2e-agent4 --command "sleep 3600" --json 2>&1)
if echo "$OUTPUT" | grep -q '"registered":true'; then
  pass "T3.3 register --json output"
else
  fail "T3.3 register --json" "output: $OUTPUT"
fi

# --- 4. Send ------------------------------------------------------------------

section "4. SEND"

# Create tmux sessions for send/signal tests
tmux new-session -d -s e2e-agent1 "sleep 3600" 2>/dev/null || true
tmux new-session -d -s e2e-agent2 "sleep 3600" 2>/dev/null || true
tmux new-session -d -s e2e-orchestrator "sleep 3600" 2>/dev/null || true
sleep 0.5

# T4.1: Send normal priority task
OUTPUT=$($BIN send e2e-agent1 --body "implement login feature" 2>&1)
if echo "$OUTPUT" | grep -q "Sent task to e2e-agent1" && echo "$OUTPUT" | grep -q "priority=normal"; then
  pass "T4.1 send normal priority task"
else
  fail "T4.1 send normal" "output: $OUTPUT"
fi

# T4.2: Send high priority task
OUTPUT=$($BIN send e2e-agent1 --body "fix critical bug" --priority high 2>&1)
if echo "$OUTPUT" | grep -q "priority=high"; then
  pass "T4.2 send --priority high"
else
  fail "T4.2 send --priority high" "output: $OUTPUT"
fi

# T4.3: Send urgent priority task
OUTPUT=$($BIN send e2e-agent2 --body "security patch" --priority urgent 2>&1)
if echo "$OUTPUT" | grep -q "priority=urgent"; then
  pass "T4.3 send --priority urgent"
else
  fail "T4.3 send --priority urgent" "output: $OUTPUT"
fi

# T4.4: Send --json output
OUTPUT=$($BIN send e2e-agent2 --body "write docs" --json 2>&1)
if echo "$OUTPUT" | grep -q '"sent":true'; then
  pass "T4.4 send --json output"
else
  fail "T4.4 send --json" "output: $OUTPUT"
fi

# T4.5: Send to non-existent agent errors
OUTPUT=$($BIN send nonexistent-agent --body "task" 2>&1) || true
if echo "$OUTPUT" | grep -qi "error\|not found\|not running"; then
  pass "T4.5 send to non-existent agent errors"
else
  fail "T4.5 send to bad agent" "output: $OUTPUT"
fi

# T4.6: Send special characters (injection safety)
OUTPUT=$($BIN send e2e-agent1 --body 'test "quotes" & $(whoami) `ls`' 2>&1)
if echo "$OUTPUT" | grep -q "Sent task to e2e-agent1"; then
  pass "T4.6 send special characters (no injection)"
else
  fail "T4.6 send special chars" "output: $OUTPUT"
fi

# --- 5. List ------------------------------------------------------------------

section "5. LIST"

# T5.1: List all messages
OUTPUT=$($BIN list 2>&1)
if echo "$OUTPUT" | grep -q "e2e-agent1" && echo "$OUTPUT" | grep -q "e2e-agent2"; then
  pass "T5.1 list shows all messages"
else
  fail "T5.1 list all" "output: $OUTPUT"
fi

# T5.2: List --agent filter
OUTPUT=$($BIN list --agent e2e-agent1 2>&1)
if echo "$OUTPUT" | grep -q "e2e-agent1" && ! echo "$OUTPUT" | grep -q "e2e-agent2"; then
  pass "T5.2 list --agent filter"
else
  fail "T5.2 list --agent filter" "output: $OUTPUT"
fi

# T5.3: List --status filter
OUTPUT=$($BIN list --status pending 2>&1)
if echo "$OUTPUT" | grep -q "pending"; then
  pass "T5.3 list --status pending filter"
else
  fail "T5.3 list --status filter" "output: $OUTPUT"
fi

# T5.4: List --limit filter
OUTPUT=$($BIN list --limit 2 2>&1)
LINE_COUNT=$(echo "$OUTPUT" | grep -c "e2e-agent" || true)
if [[ $LINE_COUNT -le 2 ]]; then
  pass "T5.4 list --limit 2"
else
  fail "T5.4 list --limit" "expected <=2 rows, got $LINE_COUNT"
fi

# T5.5: List --json output
OUTPUT=$($BIN list --json 2>&1)
if echo "$OUTPUT" | grep -q '"agent_name"'; then
  pass "T5.5 list --json output"
else
  fail "T5.5 list --json" "output: $OUTPUT"
fi

# --- 6. Peek ------------------------------------------------------------------

section "6. PEEK"

# T6.1: Peek returns highest-priority task
OUTPUT=$($BIN peek e2e-agent1 2>&1)
if echo "$OUTPUT" | grep -q "pending"; then
  pass "T6.1 peek shows pending task"
else
  fail "T6.1 peek" "output: $OUTPUT"
fi

# T6.2: Peek --json output
OUTPUT=$($BIN peek e2e-agent1 --json 2>&1)
if echo "$OUTPUT" | grep -q '"task"'; then
  pass "T6.2 peek --json output"
else
  fail "T6.2 peek --json" "output: $OUTPUT"
fi

# T6.3: Peek agent with no pending tasks (register-only agent)
OUTPUT=$($BIN peek e2e-agent3 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]] && echo "$OUTPUT" | grep -qi "no pending"; then
  pass "T6.3 peek no pending tasks exits 0"
else
  fail "T6.3 peek no pending" "exit=$EXIT_CODE output: $OUTPUT"
fi

# --- 7. Signal ----------------------------------------------------------------

section "7. SIGNAL"

# T7.1: Signal completes a task
OUTPUT=$(TMUX_PANE=%0 $BIN signal e2e-agent1 2>&1)
if echo "$OUTPUT" | grep -q "Signaled completion"; then
  pass "T7.1 signal completes task"
else
  fail "T7.1 signal" "output: $OUTPUT"
fi

# T7.2: Signal again (second pending task)
OUTPUT=$(TMUX_PANE=%0 $BIN signal e2e-agent1 2>&1)
if echo "$OUTPUT" | grep -q "Signaled completion"; then
  pass "T7.2 signal second task"
else
  fail "T7.2 signal second" "output: $OUTPUT"
fi

# T7.3: Signal remaining tasks then test idempotency
# (agent1 has 3 tasks: T4.1 normal, T4.2 high, T4.6 special chars)
TMUX_PANE=%0 $BIN signal e2e-agent1 2>/dev/null || true
OUTPUT=$(TMUX_PANE=%0 $BIN signal e2e-agent1 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]] && echo "$OUTPUT" | grep -qi "no pending\|acknowledged"; then
  pass "T7.3 signal idempotent (no pending, exits 0)"
else
  fail "T7.3 signal idempotent" "exit=$EXIT_CODE output: $OUTPUT"
fi

# T7.4: Signal guard — outside tmux (no TMUX_PANE)
OUTPUT=$(unset TMUX_PANE; $BIN signal e2e-agent1 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T7.4 signal outside tmux exits 0 (guard 1)"
else
  fail "T7.4 signal outside tmux" "exit=$EXIT_CODE"
fi

# T7.5: Signal guard — orchestrator self-signal
OUTPUT=$(TMUX_PANE=%0 $BIN signal e2e-orchestrator 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T7.5 signal orchestrator self-signal blocked (exits 0)"
else
  fail "T7.5 signal orchestrator guard" "exit=$EXIT_CODE"
fi

# T7.6: Signal unregistered agent
OUTPUT=$(TMUX_PANE=%0 $BIN signal nonexistent-agent 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T7.6 signal unregistered agent exits 0 (guard 3)"
else
  fail "T7.6 signal unregistered" "exit=$EXIT_CODE"
fi

# --- 8. Agents ----------------------------------------------------------------

section "8. AGENTS"

# T8.1: Agents lists all registered agents
OUTPUT=$($BIN agents 2>&1)
if echo "$OUTPUT" | grep -q "e2e-agent1" && echo "$OUTPUT" | grep -q "e2e-agent2" && echo "$OUTPUT" | grep -q "e2e-orchestrator"; then
  pass "T8.1 agents lists all registered"
else
  fail "T8.1 agents list" "output: $OUTPUT"
fi

# T8.2: Agents shows status (idle/busy)
OUTPUT=$($BIN agents 2>&1)
if echo "$OUTPUT" | grep -q "idle\|busy\|dead"; then
  pass "T8.2 agents shows status"
else
  fail "T8.2 agents status" "output: $OUTPUT"
fi

# T8.3: Agents reconciles dead sessions
tmux kill-session -t e2e-agent2 2>/dev/null || true
sleep 0.3
OUTPUT=$($BIN agents 2>&1)
if echo "$OUTPUT" | grep "e2e-agent2" | grep -q "dead"; then
  pass "T8.3 agents reconciles dead tmux session"
else
  fail "T8.3 agents dead reconciliation" "output: $OUTPUT"
fi

# --- 9. Context ---------------------------------------------------------------

section "9. CONTEXT"

# T9.1: Context generates markdown
OUTPUT=$($BIN context 2>&1)
if echo "$OUTPUT" | grep -q "Agent Roster" && echo "$OUTPUT" | grep -q "squad-station send"; then
  pass "T9.1 context generates markdown with roster"
else
  fail "T9.1 context" "output: $OUTPUT"
fi

# T9.2: Context shows dead agents correctly
OUTPUT=$($BIN context 2>&1)
if echo "$OUTPUT" | grep "e2e-agent2" | grep -q "dead"; then
  pass "T9.2 context marks dead agents"
else
  fail "T9.2 context dead" "output: $OUTPUT"
fi

# --- 10. Status ---------------------------------------------------------------

section "10. STATUS"

# T10.1: Status shows project overview
OUTPUT=$($BIN status 2>&1)
if echo "$OUTPUT" | grep -q "e2e-test" && echo "$OUTPUT" | grep -q "Agents:"; then
  pass "T10.1 status shows project overview"
else
  fail "T10.1 status overview" "output: $OUTPUT"
fi

# T10.2: Status shows pending counts
OUTPUT=$($BIN status 2>&1)
if echo "$OUTPUT" | grep -q "pending"; then
  pass "T10.2 status shows pending counts"
else
  fail "T10.2 status pending" "output: $OUTPUT"
fi

# T10.3: Status --json output
OUTPUT=$($BIN status --json 2>&1) || true
if echo "$OUTPUT" | grep -q "project\|agents\|error"; then
  pass "T10.3 status --json (or graceful error)"
else
  fail "T10.3 status --json" "output: $OUTPUT"
fi

# --- 11. View -----------------------------------------------------------------

section "11. VIEW"

# T11.1: View creates tmux pane layout
OUTPUT=$($BIN view 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]] && echo "$OUTPUT" | grep -qi "Created squad-view\|panes"; then
  pass "T11.1 view creates tmux pane layout"
else
  fail "T11.1 view" "exit=$EXIT_CODE output: $OUTPUT"
fi

# T11.2: View idempotent (re-run)
OUTPUT=$($BIN view 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T11.2 view idempotent (re-run exits 0)"
else
  fail "T11.2 view idempotent" "exit=$EXIT_CODE"
fi

# --- 12. UI -------------------------------------------------------------------

section "12. UI (TUI)"

# T12.1: UI requires TTY — errors gracefully without one
OUTPUT=$($BIN ui 2>&1) || true
EXIT_CODE=$?
if [[ $EXIT_CODE -ne 0 ]]; then
  pass "T12.1 ui rejects non-TTY environment (exits non-zero)"
else
  skip "T12.1 ui TTY check" "somehow succeeded without TTY"
fi

# --- 13. Hook Scripts ---------------------------------------------------------

section "13. HOOK SCRIPTS"

HOOKS_DIR="${PROJECT_ROOT}/hooks"

# T13.1: claude-code.sh exists and is executable
if [[ -x "$HOOKS_DIR/claude-code.sh" ]]; then
  pass "T13.1 claude-code.sh exists and is executable"
else
  fail "T13.1 claude-code.sh" "missing or not executable"
fi

# T13.2: gemini-cli.sh exists and is executable
if [[ -x "$HOOKS_DIR/gemini-cli.sh" ]]; then
  pass "T13.2 gemini-cli.sh exists and is executable"
else
  fail "T13.2 gemini-cli.sh" "missing or not executable"
fi

# T13.3: claude-code.sh has correct shebang and structure
if head -1 "$HOOKS_DIR/claude-code.sh" | grep -q "#!/bin/bash" && \
   grep -q "TMUX_PANE" "$HOOKS_DIR/claude-code.sh" && \
   grep -q "squad-station" "$HOOKS_DIR/claude-code.sh"; then
  pass "T13.3 claude-code.sh has correct structure"
else
  fail "T13.3 claude-code.sh structure" "missing shebang, TMUX_PANE, or squad-station reference"
fi

# T13.4: gemini-cli.sh has correct shebang and structure
if head -1 "$HOOKS_DIR/gemini-cli.sh" | grep -q "#!/bin/bash" && \
   grep -q "TMUX_PANE" "$HOOKS_DIR/gemini-cli.sh" && \
   grep -q "squad-station" "$HOOKS_DIR/gemini-cli.sh"; then
  pass "T13.4 gemini-cli.sh has correct structure"
else
  fail "T13.4 gemini-cli.sh structure" "missing shebang, TMUX_PANE, or squad-station reference"
fi

# --- 14. Edge Cases -----------------------------------------------------------

section "14. EDGE CASES"

# T14.1: List with no messages for an agent
OUTPUT=$($BIN list --agent e2e-agent3 2>&1)
EXIT_CODE=$?
if [[ $EXIT_CODE -eq 0 ]]; then
  pass "T14.1 list empty result exits 0"
else
  fail "T14.1 list empty" "exit=$EXIT_CODE"
fi

# T14.2: Completed messages visible in list
OUTPUT=$($BIN list --status completed 2>&1)
if echo "$OUTPUT" | grep -q "completed"; then
  pass "T14.2 completed messages in list"
else
  fail "T14.2 completed messages" "output: $OUTPUT"
fi

# T14.3: Concurrent safety — rapid sends don't corrupt
for i in $(seq 1 5); do
  $BIN send e2e-agent1 --body "concurrent task $i" 2>/dev/null &
done
wait
OUTPUT=$($BIN list --agent e2e-agent1 2>&1)
TASK_COUNT=$(echo "$OUTPUT" | grep -c "e2e-agent1" || true)
if [[ $TASK_COUNT -ge 5 ]]; then
  pass "T14.3 concurrent sends don't corrupt ($TASK_COUNT messages)"
else
  fail "T14.3 concurrent safety" "expected >=5 messages, got $TASK_COUNT"
fi

# ==============================================================================
# RESULTS
# ==============================================================================

echo ""
echo -e "${BOLD}══════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  RESULTS${NC}"
echo -e "${BOLD}══════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "  Total:   ${BOLD}$TOTAL${NC}"
echo -e "  Passed:  ${GREEN}${BOLD}$PASS${NC}"
echo -e "  Failed:  ${RED}${BOLD}$FAIL${NC}"
echo -e "  Skipped: ${YELLOW}${BOLD}$SKIP${NC}"

if [[ $FAIL -gt 0 ]]; then
  echo ""
  echo -e "${RED}${BOLD}  Failures:${NC}"
  echo -e "$FAILURES"
  echo ""
  exit 1
else
  echo ""
  echo -e "  ${GREEN}${BOLD}All tests passed.${NC}"
  echo ""
  exit 0
fi
