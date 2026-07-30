# Acceptance test catalog

The CSV matrix is the authoritative checklist. This document defines release-level acceptance scenarios.

## Connection acceptance

- Connect with Peak present, absent and disconnected mid-session.
- Select input/output separately.
- No stale command is sent after reconnection.
- Ambiguous port names require explicit user selection.
- Device/session diagnostics are exportable.

## Live-control acceptance

- Drag a verified CC control rapidly; UI remains responsive and final Peak value matches.
- Move physical control; UI updates without echo loop.
- Move two NRPN controls simultaneously; raw log shows serialized selector/data sequences.
- Undo/redo results in correct Peak values.
- Unverified controls cannot send from production build.

## State acceptance

- Global setting change never marks patch dirty.
- Hardware patch change while clean resynchronizes.
- Hardware patch change while dirty preserves or explicitly resolves local work.
- Unknown values are visibly unknown, not zero/default.

## SysEx acceptance

- Opaque import/export is byte-identical.
- Truncated/wrong identity messages are rejected and never sent.
- Unchanged decode/encode is byte-identical.
- One verified field edit preserves all unrelated bytes.
- Edit-buffer send is distinct from stored-memory write.

## Stored-write acceptance

- Destination and existing patch identity are visible.
- Backup is created before write.
- Confirmation cannot be bypassed by keyboard accident or double click.
- Post-write redump verifies destination content.
- Rollback restores original bytes/state.
- Audit log contains backup/sent/redump hashes.
