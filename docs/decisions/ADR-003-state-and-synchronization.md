# ADR-003: State and synchronization model

Status: Accepted

## State partitions

- `PatchState`: parameters saved with an individual patch.
- `GlobalSettingsState`: MIDI configuration, PatchCue, protection, tuning-table contents and other global settings.
- `DeviceSessionState`: ports, MIDI channel, connection health, exact OS build, queue state and capabilities.
- `LibrarianState`: local files, hashes, tags, notes and patch-location metadata.
- `EditorHistoryState`: undo/redo snapshots and dirty baseline.

A global parameter must never mark a patch dirty. A Program Change is not a patch-data message. A local librarian file is not proof of the current hardware edit buffer.

## Change provenance

Every change must carry:

- parameter ID
- old raw value
- new raw value
- source: UI, hardware panel, SysEx load, patch selection, undo, redo, initialization or verification tool
- monotonic event ID
- timestamp
- verification/confidence state
- whether a hardware send is requested

## Feedback suppression

Do not use a blanket "ignore matching incoming values" rule. Maintain short-lived outbound correlation records keyed by port, channel, parameter, raw value and sequence ID. A matching incoming value may confirm the send, but a later physical change must still be processed. Never retransmit a hardware-originated update by default.

## Patch changes

When a hardware Program/Bank Change is observed:

1. Stop sending pending parameter edits for the old patch.
2. Protect local dirty work by snapshot or prompt according to policy.
3. Mark hardware state as synchronizing.
4. Wait for the program-change burst to settle.
5. Request edit buffer only after that request is verified.
6. Atomically replace hardware patch state when the dump validates.
