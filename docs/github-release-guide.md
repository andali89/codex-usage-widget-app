# Publish from a new folder and create a GitHub release

This workflow keeps the development folder and its Git history intact. The new public folder starts a fresh Git history from the reviewed source snapshot. Your normal Git author name/email remain public, as requested. Nothing has been pushed to GitHub automatically.

## 1. Use the prepared files

The files for version 0.1.0 are under `src-tauri/target/release-assets/v0.1.0/` in the original development folder:

- `codex-usage-widget-v0.1.0-source.zip`: committed source files only; extract this into the new public folder.
- `Codex Usage Widget_0.1.0_x64-setup.exe`: installer to attach to the GitHub release.
- `SHA256SUMS.txt`: hashes for the prepared downloads.

The source ZIP is made with `git archive HEAD`. It excludes `.git`, private untracked design references, installed dependencies, application state, validation logs, and build outputs. Do not copy the whole development directory with File Explorer: that would also copy ignored local files. The source ZIP is for development; end users should download the setup executable.

## 2. Extract into a new folder

Open PowerShell in the **original repository root**. Change the example destination if needed; it must not already exist. These commands copy your configured Git identity to the new repository without printing it:

```powershell
$widgetSourceZip = (Resolve-Path 'src-tauri/target/release-assets/v0.1.0/codex-usage-widget-v0.1.0-source.zip').Path
$widgetPublicFolder = 'D:\projects\codex-usage-widget-public'
$widgetAuthorName = git config user.name
$widgetAuthorEmail = git config user.email
if (-not $widgetAuthorName -or -not $widgetAuthorEmail) { throw 'Set your Git name/email before continuing.' }
if (Test-Path -LiteralPath $widgetPublicFolder) { throw 'Choose a new folder to avoid mixing existing files.' }
Expand-Archive -LiteralPath $widgetSourceZip -DestinationPath $widgetPublicFolder
Set-Location -LiteralPath $widgetPublicFolder
git init -b main
git config user.name "$widgetAuthorName"
git config user.email "$widgetAuthorEmail"
git add .
git status --short
git diff --cached --stat
```

Review the staged filenames, then commit and rerun the dependency-free audit:

```powershell
git commit -m "chore: publish initial public preview"
node scripts/audit-publication.mjs
```

The audit prints redacted findings for review; exit code zero does not certify that everything is safe. The known proxy-validation fixture is documented in the [audit report](publication-audit.md). The old development commit IDs mentioned in historical reports remain local provenance; they will not exist in the fresh public repository.

## 3. Create an empty GitHub repository and push

On GitHub, create a repository named `codex-usage-widget` under your account and choose **Public**. Leave the README, license, and `.gitignore` initialization options unchecked because these files already exist in the source copy. Replace `YOUR-USERNAME` below, then run the commands from the **new public folder**:

```powershell
git remote add origin https://github.com/YOUR-USERNAME/codex-usage-widget.git
git push -u origin main
```

Complete Git's normal GitHub authentication when prompted. This uploads only committed source/history, not the installer or ignored files. Verify that the repository shows the expected README and source files. This follows GitHub's [existing-code publication instructions](https://docs.github.com/en/migrations/importing-source-code/using-the-command-line-to-import-source-code/adding-locally-hosted-code-to-github).

## 4. Create the version 0.1.0 prerelease

From the new public folder, tag the commit that contains the reviewed source:

```powershell
git status --short
git tag -a v0.1.0 -m "Windows x64 public preview"
git push origin v0.1.0
```

The status output should be empty. Do not reuse the prepared installer if you have changed application code after extracting the source; rebuild it first.

On GitHub, open **Releases → Draft a new release**, select tag `v0.1.0`, and use the title **v0.1.0 — Windows x64 public preview**. Paste [the prepared notes](v0.1.0-release-notes.md). Attach all three prepared files from the **original folder's** release-assets directory: the setup executable, reviewed source ZIP, and `SHA256SUMS.txt`. The checksum file describes these exact downloads. GitHub's additional automatic source archives have different archive bytes, so their checksums need not match the prepared ZIP. Keep build artifacts out of source commits.

Select **This is a pre-release**, save the draft, check the attachments and known limitations, then click **Publish release**. The application is unsigned; notification delivery, Windows 10 coverage, and the actual Photos normal/maximized reproduction are unresolved validation items. This is a public preview, not a fully validated stable release. GitHub documents this flow in [Managing releases](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository?tool=webui).

Users can compare the installer checksum with:

```powershell
Get-FileHash -LiteralPath '.\Codex Usage Widget_0.1.0_x64-setup.exe' -Algorithm SHA256
```

After publication, update README's download section with the actual release URL. A later documentation commit does not require moving the published version tag.

## 5. Build later releases

In the new public folder, install the existing pinned project dependencies as described in [README](../README.md), update version numbers in the npm/Cargo/Tauri manifests and their lockfiles together, and commit the intended release changes. Then run:

```powershell
npm.cmd run audit:publication
npm.cmd test
npm.cmd run rust:fmt
npm.cmd run rust:check
npm.cmd run rust:test
npm.cmd run release:build
```

Review all audit findings and check failures before tagging. `release:build` uses an isolated Cargo target, remaps local user/workspace compiler paths, checks that those paths are absent from the resulting native executable, and stages the installer/checksum under `src-tauri/target/release-assets/v<VERSION>/`. It changes no global toolchain or machine environment and does not install the app. The initial remapped build recompiles dependencies into its separate cache. Refresh release notes, exercise the important Windows behaviors, create the next version tag, and repeat the draft-release upload flow. Do not move an already published version tag to different code.
