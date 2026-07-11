# Helper binary paths (macOS and Windows)

The packaged helper is named **`llm-notch-hook`** in the Tauri bundle (`externalBin`). The Cargo package remains `notch-hook`; its binary target is `llm-notch-hook`.

## macOS

### Bundled app

```
/Applications/llm_notch.app/Contents/MacOS/llm-notch-hook
```

The preparation artifact is `src-tauri/binaries/llm-notch-hook-<target>`. Tauri chooses its final bundle location. Universal builds require preparing/combining both architectures. Release signing must cover the app and embedded helper; local builds are unsigned.

### Developer / cargo build

```bash
# Debug
./target/debug/llm-notch-hook

# Release
./target/release/llm-notch-hook
```

Set for hook testing:

```bash
export LLM_NOTCH_HOOK_BIN="/absolute/path/to/target/debug/llm-notch-hook"
```

Templates invoke `llm-notch-hook` by default — symlink or PATH alias during development:

```bash
ln -sf "$(pwd)/target/debug/llm-notch-hook" ./llm-notch-hook
export PATH="$(pwd):$PATH"
```

### User hook wrapper location (recommended)

```
~/.cursor/hooks/llm-notch-hook-wrapper.sh
```

Project-relative path (repo hooks):

```
<repo>/integrations/wrappers/llm-notch-hook-wrapper.sh
```

## Windows

### Bundled app

```
C:\Program Files\llm_notch\llm-notch-hook.exe
```

Per-user install variant:

```
%LOCALAPPDATA%\Programs\llm_notch\llm-notch-hook.exe
```

### Developer / cargo build

```powershell
# Debug
.\target\debug\llm-notch-hook.exe

# Release
.\target\release\llm-notch-hook.exe
```

Override:

```powershell
$env:LLM_NOTCH_HOOK_BIN = "C:\dev\llm_notch\target\debug\llm-notch-hook.exe"
```

### PowerShell wrapper

```
%USERPROFILE%\.cursor\hooks\llm-notch-hook-wrapper.ps1
```

Invoke with:

```text
pwsh -NoProfile -File "%USERPROFILE%\.cursor\hooks\llm-notch-hook-wrapper.ps1" -Source cursor -VendorEvent sessionStart
```

## Runtime descriptor (helper discovers host)

| Platform | Typical path |
|----------|--------------|
| macOS | User-local app data resolved by `directories::ProjectDirs`, under `runtime/descriptor.json` |
| Windows | User-local app data resolved by `directories::ProjectDirs`, under `runtime/descriptor.json` |

Exact paths are assigned by the desktop host at startup. Hooks **must not** embed descriptor paths.

## IPC endpoint (helper discovers host)

| Platform | Transport |
|----------|-----------|
| macOS | Unix domain socket alongside descriptor |
| Windows | Named pipe alongside descriptor |

## PATH recommendations

| Scenario | Recommendation |
|----------|----------------|
| Dashboard-installed integration | Installer writes absolute helper path in hook command |
| Manual dev | `LLM_NOTCH_HOOK_BIN` env in shell profile **not** required; prefer absolute path in hook |
| CI / tests | Inject `LLM_NOTCH_HOOK_BIN` in test harness only |

## Signature verification

Production builds should be Developer ID signed (macOS) or Authenticode signed (Windows). Users can verify:

```bash
# macOS
codesign --verify --deep --strict /Applications/llm_notch.app
spctl --assess --type execute /Applications/llm_notch.app/Contents/MacOS/llm-notch-hook
```

```powershell
# Windows
Get-AuthenticodeSignature "C:\Program Files\llm_notch\llm-notch-hook.exe"
```

Unsigned local development binaries are expected. Do not distribute them as signed releases.
