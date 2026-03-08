#!/bin/bash
# .claude/hooks/pre-tool-edit.sh
# PreToolUse hook for Edit: validates edits and adds architecture reminders
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Block edits outside the project directory
PROJECT_DIR=$(echo "$INPUT" | jq -r '.cwd')
case "$FILE_PATH" in
  "$PROJECT_DIR"/*)
    ;;
  *)
    jq -n '{
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "Edit target is outside the project directory"
      }
    }'
    exit 0
    ;;
esac

jq -n '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "allow",
    permissionDecisionReason: "Edit checks passed",
    additionalContext: "Pre-edit checks: (1) Has the file been read first? (2) Maintain crate dependency flow (cli/server → analyzer → core). (3) If crates/core/, preserve WASM compatibility. (4) If changing public API/data model, must be covered by existing design. (5) Follow existing codebase patterns."
  }
}'
