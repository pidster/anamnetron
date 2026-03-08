#!/bin/bash
# .claude/hooks/pre-tool-write.sh
# PreToolUse hook for Write: validates new file creation and adds architecture reminders
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Block writes outside the project directory
PROJECT_DIR=$(echo "$INPUT" | jq -r '.cwd')
case "$FILE_PATH" in
  "$PROJECT_DIR"/*)
    ;;
  *)
    jq -n '{
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "Write target is outside the project directory"
      }
    }'
    exit 0
    ;;
esac

jq -n '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "allow",
    permissionDecisionReason: "Write checks passed",
    additionalContext: "Pre-write checks: (1) Is a new file truly necessary or should an existing file be edited? (2) Respect crate dependency flow (cli/server → analyzer → core). (3) If in crates/core/, no platform-specific deps (must compile to WASM). (4) If new public API/trait/data model, design must be validated first."
  }
}'
