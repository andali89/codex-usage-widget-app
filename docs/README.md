# Project documentation

Store design reports, implementation reports, plans, investigation notes, validation records, and architecture decisions created during Codex work in this directory. Follow the documentation conventions in [AGENTS.md](../AGENTS.md).

## Current references

- [GitHub publication and release guide](github-release-guide.md): automated Windows installer/portable draft releases from the `release` branch, plus the earlier manual source-publication procedure.
- [Publication audit](publication-audit.md): tracked-file/history review, privacy findings, release artifacts, and verification limits.
- [Version 0.1.1 release notes](v0.1.1-release-notes.md): source-prepared defaults update; untested and not yet published.
- [Version 0.1.0 release notes](v0.1.0-release-notes.md): prepared public-preview description and known limitations.

- [Proxy, storage and compact periods](proxy-storage-periods-implementation-report.md): proxy settings, EXE/AppData/custom configuration migration and fallback, HUD/Edge period visibility, and validation evidence.

- [Layout polish implementation](layout-polish-implementation-report.md): compact spacing, Edge docking, optional quota badges, stable Edge/HUD sizes, richer hover details, centered HUD click popups, expandable credit rows, Floating close-to-tray, artwork/native coordination, and validation.
- [UI/UX design report](codex-widget-design-report.md): canonical design specification.
- [Project kickoff prompt and goal](project-kickoff-prompt.md): copyable instructions and completion objective for implementing the complete application in stages.
- The supplied design board and HUD/Edge reference images are private, local-only design inputs and are not distributed.
- [Implementation and evidence](implementation-report.md): protocol-based CLI compatibility and version diagnostics, phased checklist, core/four modes/reminders/lifecycle, widget click timing/countdowns/content sizing, dependency patch, NSIS install/upgrade checks, stress measurements, compatibility gates, and AC-01–AC-40 status.
- [Implementation handoff](implementation-handoff.md): stopped-session worktree, final checks, artifact provenance, remaining work and resumption instructions.
- [Installed notification failure](notification-error-investigation.md): reproducible `ToastNotifier.Setting()` error, attempted fixes, evidence and investigation handoff.
- [Project overview and development setup](../README.md): requirements, usage, limitations, license, setup and build commands.
- [Environment setup and validation](development-environment-setup.md): historical setup evidence, tested toolchain, presentation fixture, native validation commands, and installer smoke check.
- [Codex protocol reference](../schemas/codex/README.md): versioned schema provenance and regeneration instructions.

Add links here as maintained reports are created. Update an existing topic's report where possible; do not duplicate the canonical design report.
