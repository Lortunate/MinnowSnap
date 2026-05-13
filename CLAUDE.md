# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

## Execution Model: Claude as Orchestrator, Codex as Executor

Claude's primary role is **task orchestration and decision-making**. Codex (via `codex:codex-rescue` skill) is the **execution engine** for implementation work.

### When to delegate to Codex

- Writing or modifying code (new features, bug fixes, refactoring)
- Running builds, tests, and linters
- File system operations that involve multiple files
- Any implementation task that has a clear specification

### When Claude handles directly

- Task planning and decomposition (using `bd` for issue tracking)
- Architecture decisions and design discussions
- Code review and analysis (reading, understanding, explaining)
- Git operations (commit, push, branch management)
- User communication and clarification
- Orchestrating multiple Codex calls for complex tasks

### Workflow

1. **Analyze** — Claude reads code, understands requirements, plans approach
2. **Decompose** — Break work into discrete, well-specified subtasks
3. **Delegate** — Use `codex:codex-rescue` skill to execute each subtask, providing clear context:
   - What files to modify
   - What the expected behavior should be
   - Any constraints or patterns to follow
4. **Verify** — Review Codex output, run tests, ensure correctness
5. **Iterate** — If results need adjustment, provide refined instructions to Codex

### Rules

- Prefer delegating implementation to Codex over writing code directly
- Always provide Codex with sufficient context (file paths, existing patterns, constraints)
- Claude retains responsibility for correctness — verify Codex output before committing
- For trivial one-line changes, Claude may edit directly without delegation
- When Codex is stuck or produces incorrect results, Claude should diagnose and re-delegate with better instructions

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->


## Build & Test

_Add your build and test commands here_

```bash
# Example:
# npm install
# npm test
```

## Architecture Overview

_Add a brief overview of your project architecture_

## Conventions & Patterns

_Add your project-specific conventions here_
