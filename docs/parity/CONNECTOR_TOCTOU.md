# Connector apply TOCTOU mitigations

**Status:** `TOCTOU_P0_FIXED: true` (with documented residual risk)

Connector apply/rollback touches user config files under scope roots. Advisory per-target locks and path validation alone cannot eliminate all time-of-check/time-of-use races when parent directories can be swapped to symlinks or junctions between operations.

## Layered mitigations (Windows + Unix)

1. **Per-target advisory lock** — serializes concurrent apply on the same file; does not pin directory entries.
2. **Repeated `revalidate_locked_target`** — full scope-relative path resolution plus parent-chain `lstat` (reject symlinks, Windows reparse points/junctions, Unix fifos/sockets) immediately before each mutating step: baseline read, backup read/create, temp create, atomic replace, rollback restore.
3. **Exclusive create** — backup and temp files use `create_new` / `O_EXCL` semantics; no separate `exists()` then create.
4. **Single-buffer hash/restore** — `read_and_hash` reads once; the same bytes are hashed and written/restored (no separate path re-read for verify vs restore).
5. **Hardlink rejection** — regular-file targets with `nlink > 1` (Unix) or reparse attributes (Windows) are rejected.

## Residual risk (honest)

Without Windows handle-relative APIs (`FILE_OPEN_REPARSE_POINT` / `OBJ_DONT_REPARSE` / `NtCreateFile` with full path pinning), a privileged local attacker racing the apply window could still swap a parent directory between revalidation and open on some code paths that use path strings. Mitigations narrow that window to the minimum practical in pure Rust std + existing `windows` crate usage; full elimination would require a larger native I/O layer.

Renderer-supplied `selectedDisplayPaths` are intersected with the stored plan only; unknown paths are rejected and never used for filesystem access.
