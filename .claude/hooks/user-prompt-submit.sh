#!/bin/bash
# .claude/hooks/user-prompt-submit.sh
# UserPromptSubmit hook: adds orchestrator delegation guidance as context
INPUT=$(cat)

jq -n '{
  hookSpecificOutput: {
    hookEventName: "UserPromptSubmit",
    additionalContext: "Delegation guidance: Code changes → implementer (plan mode). Architecture questions → architect. Code review → reviewer. Test writing → test-writer. Simple questions or explicit direct-action requests may be handled directly."
  }
}'
