#!/bin/bash
# notify_hook.sh - Claude Code Hook → Telegram API (direct)
# Claude Code truyền JSON qua stdin, không phải arguments

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/.env"

# Đọc JSON từ stdin (Claude Code hook input)
HOOK_INPUT=$(cat)

# Extract message and event from JSON input
raw_message=$(echo "$HOOK_INPUT" | jq -r '.message // .last_message // "Claude Code Notification"')
hook_event=$(echo "$HOOK_INPUT" | jq -r '.hook_event_name // .type // "Unknown"')

# Extract project name from cwd
project_cwd=$(echo "$HOOK_INPUT" | jq -r '.cwd // empty')
if [[ -n "$project_cwd" ]]; then
  project_name=$(basename "$project_cwd")
else
  project_name="unknown"
fi

# Fallback if jq failed or returned null
if [ "$raw_message" = "null" ]; then
  raw_message="Claude Code Notification"
fi

# Customize message based on event type
case "$hook_event" in
  "SessionStart")
    formatted_message="<b>[$project_name]</b> Session started 🚀"
    ;;
  "SessionEnd")
    formatted_message="<b>[$project_name]</b> Session completed ✅"
    ;;
  "Stop")
    # Try to extract actual message from transcript if available
    transcript_path=$(echo "$HOOK_INPUT" | jq -r '.transcript_path // empty')
    transcript_message=""

    if [[ -n "$transcript_path" && -f "$transcript_path" ]]; then
       # Wait for transcript to be flushed (Stop hook fires before last entry is written)
       sleep 0.5
       transcript_message=$(grep '"type":"assistant"' "$transcript_path" | tail -n 1 | jq -r '.message.content[] | select(.type=="text") | .text' 2>/dev/null)
    fi

    if [[ -n "$transcript_message" ]]; then
        formatted_message="<b>[$project_name]</b> 🏁 $transcript_message"
    elif [[ "$raw_message" != "Claude Code Notification" && -n "$raw_message" ]]; then
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

# Send to Telegram API
curl -s -X POST "https://api.telegram.org/bot${TELE_TOKEN}/sendMessage" \
  -H "Content-Type: application/json" \
  -d "$(jq -n \
    --arg chat_id "$TELE_CHAT_ID" \
    --arg text "$formatted_message" \
    '{
       chat_id: $chat_id,
       text: $text,
       parse_mode: "HTML",
       disable_web_page_preview: true
    }'
  )" > /dev/null 2>&1 &
