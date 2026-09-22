# Multi-Agent Development Workflow

This repository uses a multi-agent development workflow.

The agents involved are:

- Codex Planner
- Antigravity Implementer
- Codex Reviewer

The workflow is:

Codex Plan
→ Antigravity Implement
→ Codex Review
→ Antigravity Fix if needed
→ Codex Review again
→ Complete

---

# 1. General Rules

Before doing any work:

- Inspect the repository first.
- Understand the existing architecture before proposing changes.
- Detect the project's language, framework, package manager, build system, test framework, and coding conventions.
- Read relevant configuration files and documentation.
- Follow existing patterns whenever possible.
- Do not assume a specific framework or project structure.
- Do not introduce unnecessary dependencies.
- Do not rewrite unrelated code.
- Do not perform unrelated refactors.
- Preserve existing behavior unless the task explicitly requires changing it.
- Prefer the smallest safe change that satisfies the requirement.
- Never expose secrets, credentials, tokens, or private configuration.
- Never delete user data or destructive resources unless explicitly required.
- Use Git diff/status to understand what changed.

Project-specific instructions always take priority over these generic rules.

---

# 2. Repository Discovery

Before planning or implementation, inspect enough of the repository to understand:

- project purpose
- application entry points
- major modules
- architecture
- dependencies
- package manager
- build commands
- development commands
- lint commands
- type-check commands
- test commands
- deployment-related configuration
- environment variable usage
- existing coding conventions

Look for files such as:

- README
- package manifests
- lock files
- build configuration
- CI configuration
- test configuration
- lint configuration
- type-check configuration
- environment examples
- existing agent/project instructions

Do not assume commands. Discover them from the repository.

---

# 3. Codex Planner

Role:
Software Architect / Technical Planner

Codex Planner must NOT implement the task.

Codex Planner responsibilities:

1. Read the user's request.
2. Inspect the repository.
3. Understand the relevant existing architecture.
4. Identify the smallest safe implementation path.
5. Produce an implementation-ready plan for Antigravity.

The plan should contain:

## Goal

Clearly describe what must be achieved.

## Existing Architecture

Describe only the parts relevant to the task.

## Files / Components

Identify files, modules, services, components, schemas, APIs, tests, or configuration likely to require changes.

Do not invent files unless creating a new file is necessary.

## Implementation Steps

Provide ordered and concrete implementation steps.

Each step should explain:

- what should change
- where it should change
- expected behavior

## Edge Cases

Identify important edge cases and failure scenarios.

## Risks

Identify possible:

- regressions
- security issues
- compatibility issues
- data integrity issues
- performance concerns
- concurrency/state issues

when relevant.

## Validation

Determine the appropriate validation based on the repository.

Examples may include:

- tests
- lint
- type checking
- build
- static analysis
- integration tests
- manual verification

Do not assume all projects have all of these.

## Acceptance Criteria

Define objective conditions that indicate the task is complete.

Planner restrictions:

- Do not modify source code.
- Do not implement the feature.
- Do not perform speculative refactoring.
- Do not expand the scope unnecessarily.

The output must be specific enough that another agent can implement it without redesigning the solution.

---

# 4. Antigravity Implementer

Role:
Implementation Engineer

Antigravity must implement the approved Codex plan.

Before editing:

1. Read the original task.
2. Read the Codex plan.
3. Inspect the relevant repository files.
4. Verify that the plan still matches the actual repository.

If the repository materially differs from the plan:

- adapt conservatively
- preserve the original intent
- explain significant deviations

Implementation rules:

- Follow existing architecture.
- Follow existing coding style.
- Follow established naming conventions.
- Reuse existing utilities and abstractions when appropriate.
- Modify only what is necessary.
- Avoid unrelated cleanup.
- Avoid unnecessary dependencies.
- Preserve backward compatibility when possible.
- Do not silently remove functionality.
- Handle relevant errors and edge cases.
- Keep changes focused on the requested task.

After implementation:

1. Inspect the Git diff.
2. Run the relevant verification commands discovered from the repository.
3. Fix failures caused by the implementation.
4. Do not hide or ignore failing checks.

Verification may include:

- tests
- lint
- type checking
- build
- integration checks

Only run commands that make sense for the current project.

Final implementation report should include:

- files changed
- implementation summary
- validation performed
- validation results
- known limitations
- deviations from the plan
- anything that could not be verified

---

# 5. Codex Reviewer

Role:
Senior Code Reviewer

Codex Reviewer must review the implementation but must NOT modify source code.

Review inputs:

- original user request
- Codex plan
- acceptance criteria
- current Git diff
- relevant existing code
- validation/test results

Review for:

- functional correctness
- incomplete requirements
- logic errors
- regressions
- architecture violations
- security vulnerabilities
- unsafe data handling
- broken API contracts
- state management issues
- concurrency problems
- validation errors
- incorrect error handling
- compatibility problems
- unnecessary scope expansion
- failing tests/build/type checks
- missing important tests

Findings must be classified as:

## BLOCKING

An issue that must be fixed before the task can be considered complete.

Examples:

- requirement not implemented
- incorrect behavior
- likely regression
- security vulnerability
- data corruption risk
- broken build
- failing relevant tests caused by the change
- serious architecture violation
- broken API contract

## ADVISORY

A non-blocking suggestion.

Examples:

- optional refactoring
- naming preference
- style preference
- possible future optimization
- nice-to-have test
- minor cleanup

ADVISORY findings must NOT cause rejection.

For every BLOCKING finding provide:

- file
- relevant location
- issue
- why it matters
- expected behavior
- precise fix instruction

Reviewer decision:

- ACCEPT if there are no BLOCKING issues.
- REJECT if one or more BLOCKING issues exist.

Reviewer restrictions:

- Do not modify source code.
- Do not implement fixes.
- Do not reject based only on style preferences.
- Do not expand the task scope.
- Do not invent new requirements.

---

# 6. Antigravity Fix Cycle

When Codex Reviewer returns BLOCKING issues:

Antigravity must:

1. Read the entire review.
2. Fix all BLOCKING findings.
3. Avoid unrelated changes.
4. Re-run relevant validation.
5. Inspect the resulting Git diff.
6. Report what was fixed.

ADVISORY items should generally be left unchanged unless:

- they are trivial and directly related
- fixing them introduces no additional risk
- they are necessary for correctness

After fixes, return the implementation to Codex Reviewer.

---

# 7. Review Loop

The expected workflow is:

Codex Planner
→ Antigravity Implementer
→ Codex Reviewer

If ACCEPT:

→ Complete

If REJECT:

→ Antigravity Fix
→ Codex Reviewer again

Limit repeated review/fix cycles.

If the same blocking issue remains unresolved after multiple attempts, escalate to the user rather than continuing indefinitely.

---

# 8. Definition of Done

A task is complete only when:

- requested behavior is implemented
- acceptance criteria are satisfied
- no BLOCKING review findings remain
- relevant verification passes
- no obvious unrelated regressions are introduced
- changes remain within the requested scope

Do not require perfection beyond the task's actual requirements.

---

# 9. Production Deployment Procedure

Production deployment is manual and user-triggered. AI agents must not pull, rebuild, swap, restart, or deploy production automatically after a push.

## Planner Packaging

After implementation is accepted and ready for release, Codex Planner must:

1. Run the relevant checks and create the production artifact using the repository's established build process.
2. Name the artifact with the target commit, for example `dist-<short-commit>-deploy.zip`.
3. Report the commit hash, artifact name, checksum, and any known limitations.
4. Wait for the user to upload the artifact to cPanel and explicitly request deployment.

## User Upload and Deploy Trigger

The user uploads the packaged artifact to the cPanel account. Only an explicit user instruction such as `Deploy` authorizes the production deployment.

Until that instruction is received, agents may inspect status but must not change the live production `dist` directory, restart Node.js, or run deployment commands.

## Deployment Steps

When the user explicitly requests deployment, the responsible agent must:

1. Verify the uploaded artifact, commit identity, archive integrity, and expected files (`index.js`, `public/index.html`, and `public/assets`).
2. Create a timestamped rollback backup of the current production `dist` before making changes.
3. Extract and validate the new artifact in a staging directory under the application root.
4. Perform an atomic swap into the production `dist` directory.
5. Trigger the cPanel Node.js restart using the established restart signal.
6. Verify the live application and the requested change in production.
7. Remove only temporary artifacts, staging directories, and deployment bridges created by this deployment. Preserve the timestamped rollback backup.

## Deployment Safety Boundaries

- Do not touch WordPress paths, including `public_html`, `wordpress-backups`, or `softaculous_backups`.
- Do not delete existing rollback backups.
- Do not deploy from an unverified or partially uploaded artifact.
- If verification or the swap fails, preserve the current live version and report the blocker; do not claim deployment success.
- Keep Git push, artifact upload, and production deployment as separate, explicitly reported stages.
