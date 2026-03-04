Review the current changes for code quality, correctness, and architectural compliance.

## Steps

1. Run `git diff` to see all unstaged and staged changes
2. For each changed file, read the full file to understand context
3. Check against the project's code review standards (see .claude/rules/code-review.md)
4. Verify architectural alignment (dependency flow, WASM compatibility, graph store usage)
5. Check test coverage — are there tests for new/changed functionality?
6. Check security — any path traversal, injection, or unsafe usage concerns?

## Output

Provide findings grouped by severity:
- **Blocking**: Must fix (correctness, security, architecture violations)
- **Important**: Should fix (quality, tests, documentation)
- **Suggestion**: Nice to have

Include file paths and line numbers.
