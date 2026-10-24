# Project instructions

## Project context

Codex Usage Widget is a Windows 10/11 desktop utility built with Tauri 2, Rust, React, TypeScript, Vite, and Tailwind CSS. Read [README.md](README.md) for setup and [the design report](docs/codex-widget-design-report.md) for intended behavior before changing a relevant subsystem.

The application is under phased implementation. Do not describe planned widget modes, usage reads, reminders, persistence, or themes as implemented without inspecting the code and validating the behavior. Manifests, lockfiles, and source code establish the actual implementation; reports describe requirements and recorded evidence in `docs/implementation-report.md`.

## Documentation directory

**Maintain `docs/` as the project documentation folder. Store documents created during Codex work there**, including design reports, implementation reports, implementation plans, investigation notes, test/validation reports, and architecture decisions. Do not scatter new reports across the repository root or source directories.

- Keep [docs/README.md](docs/README.md) as a short index of maintained documents.
- Use descriptive Markdown filenames, such as `tray-hover-design.md` or `theme-implementation-report.md`. Use a date prefix for a time-specific investigation or validation record when useful.
- Update an existing report for the same topic instead of creating competing copies. For substantial implementation work, create or update a concise implementation report with the changes, validation performed, results, and remaining limitations. Small edits do not require a separate report.
- Distinguish planned, implemented, tested, and manually unverified behavior. Record actual commands and outcomes; never mark a check as passed unless it ran successfully.
- Keep reports focused on durable decisions and evidence, not raw terminal transcripts. Do not include credentials, account identifiers, private usage responses, or other sensitive data.
- Keep the root `README.md` and `AGENTS.md` in place. Component-specific READMEs may remain beside the component they explain.
- [docs/codex-widget-design-report.md](docs/codex-widget-design-report.md) is the canonical design report; its supplied reference board is private, local-only material excluded from the public repository. Do not duplicate the canonical report or publish private reference images. If relocating a document or asset, update all incoming and relative links in the same change.

## Repository map

- `src/`: React presentation, TypeScript, and styles; `src/assets/` contains selected production artwork. Generated artwork masters and prompt provenance live in `docs/assets/` and the layout-polish report.
- `src-tauri/`: Rust application, native configuration, capabilities, and icons.
- `src-tauri/vendor/auto-launch/`: Maintained MIT-licensed Windows autostart dependency patch; provenance/changes are recorded in the implementation report and covered by `rust:test`/`rust:fmt`.
- `schemas/codex/`: Versioned Codex protocol schema references and regeneration guidance. Do not hand-edit generated schemas or overwrite a version using a different CLI version.
- `scripts/`: Project-local validation orchestration; generated measurements belong under `src-tauri/target/validation/`.
- `docs/`: Reports, plans, design notes, and validation records produced during development.
- `docs/assets/`: Documentation images and design references.
- `dist/`, `node_modules/`, `src-tauri/target/`, and `src-tauri/gen/`: Build, dependency, or generated outputs; do not edit these as source or commit them.

## Design and architecture constraints

- Implement four modes: Floating, Screen Edge, System Tray, and Compact HUD. Keep the tray as the recovery/control surface. Do not add an inline taskbar chip or taskbar-adjacent substitute unless the user changes the scope.
- Keep tray hover compact and read-only, approximately 260 × 76 logical pixels at default text size. Clicking opens the larger dashboard. Follow the report's focus, hover timing, placement, and tooltip-fallback rules.
- Support Light, Dark, and Follow Windows across app-rendered surfaces, with Follow Windows as default. Honor high contrast, reduced motion, text scaling, and disabled transparency.
- Rust owns authoritative state, process supervision, scheduling, persistence, reminders, and native-window coordination. React renders shared state and issues narrow validated commands; each view must not create its own quota client or scheduler.
- Verify optional Codex fields against the selected versioned schema. Launch a validated native executable with structured arguments; do not assume a shell wrapper is the app-server executable.
- Display remaining quota consistently. Missing or stale data is not zero; an elapsed reset countdown does not prove recovery. Do not infer remaining working hours, requests, or tokens from quota percentages.
- Keep authentication with the local Codex installation. Never copy credential files into the project, log secrets, issue inference requests to test usage reads, or redeem reset credits.
- Keep Tauri permissions and IPC capabilities narrow. Do not enable broad shell, filesystem, or network access for convenience.

## Development workflow

- Inspect the working tree before editing and preserve unrelated user changes. Prefer small, focused changes that follow existing patterns.
- Use the existing npm/Cargo toolchain and lockfiles. Avoid dependency upgrades unrelated to the task; update the relevant lockfile when intentionally changing dependencies.
- Run commands from the repository root. On Windows, prefer `npm.cmd`; use a PowerShell session without the user profile if profile startup fails.
- Reuse installed tooling. Follow the README's requirement to obtain confirmation before installing additional software or development tools; do not change global tooling, shell profiles, or machine configuration as a workaround.
- Keep documentation aligned with behavior changes, and report any discrepancy between the design and verified platform capabilities.

## Git commits

- During implementation, freely create local commits for completed, coherent changes without asking for permission each time. Prefer small commits at meaningful milestones rather than one large commit at the end.
- Use Conventional Commit messages: `type(scope): short imperative description`. The scope is optional. Use `feat` for new features, `fix` for bug fixes, `docs` for documentation, `refactor` for behavior-preserving code changes, `test` for tests, `style` for formatting, `build` for build/dependency changes, `ci` for automation, and `chore` for other maintenance.
- Examples: `feat(tray): add compact hover preview`, `fix(quota): preserve stale data status`, and `docs: update implementation report`.
- Before committing, inspect the diff, run the relevant checks, and review the staged diff. Stage only files or hunks belonging to the change; preserve unrelated user work and existing staged changes. Include relevant documentation and lockfile updates with their implementation.
- Record validation limitations in the implementation report and summarize created commits in the handoff. Never include secrets, generated build outputs, or dependency caches.
- This authorization covers local commits. Pushes, releases, and rewriting existing history require explicit user authorization; do not amend existing commits unless requested.

## Validation commands

Choose checks appropriate to the change; these scripts are defined in `package.json`:

| Purpose | Command |
|---|---|
| Frontend type checking | `npm.cmd run typecheck` |
| Frontend policy tests (Node built-in runner) | `npm.cmd test` |
| Redacted tracked-file/history publication audit (review findings; not a pass/fail gate) | `npm.cmd run audit:publication` |
| Frontend production build (includes type checking) | `npm.cmd run build` |
| Rust formatting check (does not rewrite files) | `npm.cmd run rust:fmt` |
| Rust compilation check | `npm.cmd run rust:check` |
| Rust behavioral tests | `npm.cmd run rust:test` |
| Explicit live read-only Codex gate | `npm.cmd run protocol:probe` |
| Isolated debug native stress check (build debug first) | `npm.cmd run native:stress` |
| Rust lint checks | `npm.cmd run rust:clippy` |
| Native application build, without installer | `npm.cmd run desktop:build` |
| Per-user NSIS installer build | `npm.cmd run desktop:package` |
| Public release build with local source paths remapped and checksum assets | `npm.cmd run release:build` |
| Guarded install/reinstall/uninstall smoke | `npm.cmd run package:check` |
| Interactive desktop development | `npm.cmd run desktop:dev` |

For documentation-only changes, check paths, links, formatting, and consistency; an application rebuild is unnecessary. For frontend changes, run the frontend build; for native changes, run the Rust checks. Use the desktop build for integration/configuration changes when applicable. Add focused behavioral tests for new logic; inspect available test tooling before choosing a framework, since the initial scaffold has no test runner.

Native tray positioning, focus, Explorer recovery, DPI scaling, OS appearance, and notifications require Windows runtime checks. A successful build alone does not validate them. Stop processes started for validation when finished. In the final handoff, state what changed, which checks passed, what could not be checked, and where the relevant report is stored.

## Instruction maintenance

Keep this file concise and update it when repository structure, commands, or established conventions change. Put detailed task-specific material in `docs/`. For instruction-file conventions, see [official OpenAI documentation](https://learn.chatgpt.com/docs/agent-configuration/agents-md).
