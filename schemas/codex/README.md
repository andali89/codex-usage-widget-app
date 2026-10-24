# Codex protocol reference

`0.163.0/` contains the unmodified JSON Schema output from the installed native Windows x64 Codex CLI **0.163.0**, generated on **2026-10-22**. There are 318 generated JSON files, including the aggregate bundles and the v2 account/rate-limit contract.

The generator was run without `--experimental`. These are protocol definitions, not captured account responses or live quota fixtures. This setup does not start an app server, read account credentials, or validate account-specific field availability. Check the generated definitions before relying on optional fields in the design report.

## Regenerate on this machine

The existing npm installation stores the native executable beneath its platform-specific package. From the repository root in PowerShell:

```powershell
$codexShim = (Get-Command codex.cmd).Source
$codexPackage = Join-Path (Split-Path -Parent $codexShim) 'node_modules\@openai\codex'
$codexExe = Join-Path $codexPackage 'node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe'
& $codexExe --version
# Confirm that this reports codex-cli 0.163.0 before regenerating this version.
& $codexExe app-server generate-json-schema --out .\schemas\codex\0.163.0
```

For a different CLI version, generate into a new version-named directory and review the contract when updating the adapter. These snapshots are regression references, not a runtime version allowlist: the widget attempts its existing read-only protocol on any native CLI version. Unknown additional fields are ignored and absent optional quota fields remain unavailable. Do not overwrite the pinned reference using a different version. Other installation methods can place `codex.exe` elsewhere; use their validated native executable path directly.

Source: [official Codex app-server documentation](https://learn.chatgpt.com/docs/app-server).
