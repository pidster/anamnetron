#!/bin/bash
# .claude/hooks/post-tool-check.sh
# PostToolUse hook for Edit|Write: adds post-edit quality reminders as context
INPUT=$(cat)

jq -n '{
  hookSpecificOutput: {
    hookEventName: "PostToolUse",
    additionalContext: "Post-edit checks: (1) Does modified code need tests? (2) Any unwrap()/expect() outside test modules? (3) Public API missing doc comments? (4) Security concerns (path traversal, injection, unsafe)?"
  }
}'
