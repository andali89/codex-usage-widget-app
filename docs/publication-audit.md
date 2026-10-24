# Publication audit and release preparation

Date: 2026-10-24. Scope: the local development repository, its reachable Git history, and the prepared Windows x64 installer. This records a heuristic confidentiality review, not a guarantee that arbitrary confidential information can be recognized automatically.

## Findings

- The baseline was commit `b5e4158`, with **416 tracked files**, **30 commits**, **763 unique historical file blobs** (756 text and 7 binary), and no configured remote or tags. The protocol schema directory contains definitions, not captured credentials or live account responses.
- No private keys, recognizable service tokens, JWTs, or actual credential assignments were found by the text scan/manual review. Its only credential-like match was the intentionally invalid proxy URL in the rejection fixture at `src-tauri/src/proxy.rs:90`.
- Private design-board/reference images are present only as ignored local files. Their filenames were absent from all reachable historical file lists. Build outputs, dependencies, validation logs, and generated native outputs are not tracked.
- Generated production artwork is intentional public material. The two tracked artwork masters were visually inspected; neither contains a private screenshot or personal data. The five current PNG assets have no text/EXIF metadata chunks. The source and vendored dependency retain their existing license files and copyright attribution.
- The existing commits contain a non-GitHub-noreply author/committer email. The user explicitly chose to keep it. Its value is not repeated in this public report. The new-folder publication workflow starts fresh public history; the original development history is preserved unchanged.
- An initial native binary contained 414 local user-home path matches from compiler diagnostics. The public release build remaps user/workspace source paths and the PDB reference rather than distributing those paths. Public artifacts exclude PDB/debug-symbol files.

## Changes made for publication

Expanded `.gitignore` for local widget settings, pending/staged state, storage-location records, credential/tool folders, and signing stores. Existing private reference exclusions remain. Normalized two historical command examples to repository-relative paths; no historical application evidence or commit objects were rewritten.

Added `npm.cmd run audit:publication`, a dependency-free Node scan of tracked working files and text objects reachable from all Git refs. It reports filenames, locations, and categories without printing matching values or Git email addresses. Findings require human review; a successful exit is not a clean-security certification. Untracked/unreachable content and arbitrary binary data are outside this scan.

Added `npm.cmd run release:build`, which uses an isolated Cargo target, process-local path-remapping flags and the existing Tauri/NSIS toolchain. It verifies that original user/workspace prefixes are absent from the native executable, then stages the installer and SHA-256 checksum. The first remapped build recompiles cached dependencies; no dependency versions, global tooling, or machine settings were changed.

The [new-folder GitHub guide](github-release-guide.md) describes source-only extraction, fresh Git initialization, pushing an empty GitHub repository, and publishing the prepared [prerelease notes](v0.1.0-release-notes.md). No repository, push, tag, or GitHub release was created automatically.

## Prepared installer

Ready-to-upload directory: `src-tauri/target/release-assets/v0.1.0/` (ignored build output).

| Item | Evidence |
| --- | --- |
| Installer | `Codex Usage Widget_0.1.0_x64-setup.exe` |
| File version / architecture | 0.1.0 / x64 |
| Bytes | 2,909,498 |
| SHA-256 | `88517af0d049b3dffcede8be9b2c734d32ded61b19e460a198b3142be965554d` |
| Signing | Unsigned (`NotSigned`) |
| Payload review | Generated NSIS file entries contain the application and selected notification PNG, plus conditional WebView2 installer handling. No local widget state or credential files are listed. |

A source ZIP is generated from the completed preparation commit with `git archive HEAD`; it contains committed files without the original `.git` directory or ignored files. `SHA256SUMS.txt` accompanies the release assets. GitHub also creates source archives from the public version tag.

## Validation and limits

- `npm.cmd run --silent audit:publication`: the complete staged preparation snapshot contained **421 tracked files**, no risky current/historical paths, and only the reviewed proxy-test match. All 16 tracked Markdown documents' local file links resolve within the tracked source tree.
- `npm.cmd test`: all 36 frontend policy/presentation tests passed.
- `node --check scripts/audit-publication.mjs` and `node scripts/audit-publication.mjs --self-test`: passed. The release PowerShell script also passed syntax parsing and an actual build run.
- `npm.cmd run release:build`: passed frontend type checking/build, native release compilation, NSIS packaging, and the executable's local source-path check. The existing default release path could not be replaced, so the isolated remapped build avoids touching it.
- Independent UTF-8/UTF-16 scans of the final native executable and setup executable found zero recognizable service tokens, private-key headers, JWTs, or local Windows user-home paths. The application PE machine type is x64 (`0x8664`); the NSIS bootstrap executable itself uses the normal 32-bit installer stub.
- The installer was built and statically inspected; it was not installed or smoke-tested during this audit. No extra full native runtime matrix or live Codex read was run.
- Notifications remain unresolved; Windows 10, accessibility/mixed-DPI, absent-WebView2 installation, and the actual Photos normal/maximized reproduction remain incompletely verified. Publish **v0.1.0 as a prerelease**, with those limitations visible.

Keep generated assets, application state, account responses, raw logs, and original private references out of source uploads. The source archive and version-control workflow are preferable to copying the entire development folder.
