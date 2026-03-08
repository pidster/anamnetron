#!/bin/bash
# .claude/hooks/post-tool-format.sh
# PostToolUse hook for Edit|Write: runs rustfmt on Rust files
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ "$FILE_PATH" == *.rs ]]; then
  rustfmt "$FILE_PATH" 2>/dev/null || true
fi

exit 0
