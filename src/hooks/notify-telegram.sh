#!/bin/bash
# notify-telegram.sh — Squad Station hook → Telegram Bot API
# Embedded in squad-station binary, written to .squad/hooks/ during init.
# Providers pass JSON on stdin. Always exits 0 (never breaks the provider).

# ── Load config ──────────────────────────────────────────────────────
# SQUAD_PROJECT_ROOT is set in the hook command by squad-station init.
PROJECT_ROOT="${SQUAD_PROJECT_ROOT:-}"
if [[ -z "$PROJECT_ROOT" ]]; then
  exit 0
fi

# Credentials from .env.squad (project root, gitignored)
if [[ -f "$PROJECT_ROOT/.env.squad" ]]; then
  source "$PROJECT_ROOT/.env.squad"
fi

# Non-sensitive config from .squad/telegram.env (auto-generated from squad.yml)
if [[ -f "$PROJECT_ROOT/.squad/telegram.env" ]]; then
  source "$PROJECT_ROOT/.squad/telegram.env"
fi

# Guard: credentials required
if [[ -z "$TELE_TOKEN" || -z "$TELE_CHAT_ID" ]]; then
  exit 0
fi

# ── Read hook input ──────────────────────────────────────────────────
HOOK_INPUT=$(cat)

# Extract fields from JSON
raw_message=$(echo "$HOOK_INPUT" | jq -r '.message // .last_message // "Notification"')
hook_event=$(echo "$HOOK_INPUT" | jq -r '.hook_event_name // .type // "Unknown"')

# Extract project name from cwd
project_cwd=$(echo "$HOOK_INPUT" | jq -r '.cwd // empty')
if [[ -n "$project_cwd" ]]; then
  project_name=$(basename "$project_cwd")
else
  project_name=$(basename "$PROJECT_ROOT")
fi

# Detect agent name (tmux session name)
agent_name=$(tmux display-message -p '#S' 2>/dev/null || echo "")

# ── Agent filter ─────────────────────────────────────────────────────
TELE_NOTIFY_AGENTS="${TELE_NOTIFY_AGENTS:-all}"
if [[ "$TELE_NOTIFY_AGENTS" != "all" ]]; then
  # Not in a tmux session → not an agent → skip
  if [[ -z "$agent_name" ]]; then
    exit 0
  fi
  # Check if agent is in the comma-separated list
  match=false
  IFS=',' read -ra AGENTS <<< "$TELE_NOTIFY_AGENTS"
  for a in "${AGENTS[@]}"; do
    a=$(echo "$a" | xargs)  # trim whitespace
    if [[ "$agent_name" == *"-$a" || "$agent_name" == "$a" ]]; then
      match=true
      break
    fi
  done
  if [[ "$match" == "false" ]]; then
    exit 0
  fi
fi

# ── Format message ───────────────────────────────────────────────────
if [ "$raw_message" = "null" ]; then
  raw_message="Notification"
fi

case "$hook_event" in
  "SessionStart")
    formatted_message="<b>[$project_name]</b> Session started 🚀"
    ;;
  "SessionEnd")
    formatted_message="<b>[$project_name]</b> Session completed ✅"
    ;;
  "Stop")
    # Try to extract message from transcript if available
    transcript_path=$(echo "$HOOK_INPUT" | jq -r '.transcript_path // empty')
    transcript_message=""
    if [[ -n "$transcript_path" && -f "$transcript_path" ]]; then
      sleep 0.5
      transcript_message=$(grep '"type":"assistant"' "$transcript_path" | tail -n 1 | jq -r '.message.content[] | select(.type=="text") | .text' 2>/dev/null)
    fi
    if [[ -n "$transcript_message" ]]; then
      formatted_message="<b>[$project_name]</b> 🏁 $transcript_message"
    elif [[ "$raw_message" != "Notification" && -n "$raw_message" ]]; then
      formatted_message="<b>[$project_name]</b> 🏁 $raw_message"
    else
      formatted_message="<b>[$project_name]</b> Response finished 🏁"
    fi
    ;;
  "Notification")
    formatted_message="<b>[$project_name]</b> $raw_message"
    ;;
  *)
    formatted_message="<b>[$project_name]</b> $hook_event: $raw_message"
    ;;
esac

# Truncate if too long (Telegram 4096 char limit)
if (( ${#formatted_message} > 4096 )); then
  formatted_message="${formatted_message:0:4080}... <i>(truncated)</i>"
fi

# ── Send to Telegram API ─────────────────────────────────────────────
# Build JSON payload — add message_thread_id only if topic_id is set
if [[ -n "$TELE_TOPIC_ID" ]]; then
  payload=$(jq -n \
    --arg chat_id "$TELE_CHAT_ID" \
    --arg text "$formatted_message" \
    --arg thread_id "$TELE_TOPIC_ID" \
    '{
       chat_id: $chat_id,
       text: $text,
       parse_mode: "HTML",
       disable_web_page_preview: true,
       message_thread_id: ($thread_id | tonumber)
    }')
else
  payload=$(jq -n \
    --arg chat_id "$TELE_CHAT_ID" \
    --arg text "$formatted_message" \
    '{
       chat_id: $chat_id,
       text: $text,
       parse_mode: "HTML",
       disable_web_page_preview: true
    }')
fi

curl -s -X POST "https://api.telegram.org/bot${TELE_TOKEN}/sendMessage" \
  -H "Content-Type: application/json" \
  -d "$payload" > /dev/null 2>&1 &
