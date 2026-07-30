# Ordered coding-agent execution checklist

Check items in order. Do not skip a gate because UI work appears easier.

## Phase 0: Repository

- [ ] Create Tauri 2 + React/TypeScript desktop app.
- [ ] Create Rust workspace crates matching the architecture plan.
- [ ] Create `peakctl` CLI.
- [ ] Add Rust and TypeScript CI.
- [ ] Copy registry/enums/schema sources into the repository.
- [ ] Run `scripts/validate_registry.py` and make it a CI gate.
- [ ] Add experimental feature flags and ensure all are off by default.

## Phase 1: Raw transport

- [ ] Implement port enumeration and explicit input/output selection.
- [ ] Define stable session/port identifiers.
- [ ] Implement input callback to bounded channel.
- [ ] Timestamp and preserve raw bytes.
- [ ] Implement JSONL session log and SHA-256 file manifest.
- [ ] Implement clean close, rescan and reconnect.
- [ ] Implement raw monitor UI and CLI output.
- [ ] Pass HV-003 and HV-004.

## Phase 2: Safe channel messages

- [ ] Implement CC encode/decode and tests.
- [ ] Allowlist only CC79 for first send test.
- [ ] Pass HV-005/HV-006.
- [ ] Implement generic NRPN state machine with CC99, CC98, CC6 and optional CC38 support.
- [ ] Put NRPN sending behind feature flag and allowlist 0:14 only.
- [ ] Capture Peak's own sequence.
- [ ] Update the device profile to match evidence.
- [ ] Pass HV-009/HV-010/HV-011.

## Phase 3: Scheduler and sync

- [ ] One output scheduler per port.
- [ ] Atomic multi-message sequence lock.
- [ ] Coalesce continuous superseded values.
- [ ] Cancel on disconnect and patch transition.
- [ ] Add queue metrics.
- [ ] Add outbound correlation records.
- [ ] Prevent hardware-originated retransmission.
- [ ] Pass HV-012/HV-013/HV-014.

## Phase 4: CC-pair research

- [ ] Build receive-only paired-CC analyzer.
- [ ] Collect boundary/adjacent captures.
- [ ] Write a codec specification with examples.
- [ ] Implement exhaustive encode/decode tests for 0-255 or documented subrange.
- [ ] Do not enable any paired control before HV-008.

## Phase 5: Domain/UI foundation

- [ ] Generate Rust and TypeScript registry types.
- [ ] Implement PatchState, GlobalSettingsState, DeviceSessionState and EditorHistoryState.
- [ ] Implement evidence status in developer diagnostics.
- [ ] Implement connection screen.
- [ ] Implement first controls: Filter Resonance and Oscillator 1 Wave.
- [ ] Add undo/redo and dirty state.
- [ ] Add incomplete-state indicator before edit-buffer sync exists.

## Phase 6: Full live control

- [ ] Work section by section, enabling only verified definitions.
- [ ] Capture missing firmware 2.1 mappings.
- [ ] Capture all current enum codes.
- [ ] Implement matrices using verified codes, never label-array indices.
- [ ] Run a parameter coverage report in CI.
- [ ] Pass relevant P1 tests.

## Phase 7: Opaque SysEx

- [ ] Implement F0/F7 framer with malformed-message handling.
- [ ] Capture Current twice and store exact bytes/hashes.
- [ ] Import/export byte-identically.
- [ ] Implement universal/manufacturer identity views without mutating bytes.
- [ ] Pass HV-015/HV-016/HV-017/HV-045.

## Phase 8: SysEx reverse engineering

- [ ] Create byte-diff and bit-mask tool.
- [ ] Capture one-parameter changes.
- [ ] Document every mapped field with dump fixtures.
- [ ] Decoder starts from immutable original bytes.
- [ ] Encoder patches only verified fields.
- [ ] Prove unchanged byte identity on varied patches.
- [ ] Prove one-field edits change only expected bytes.
- [ ] Pass HV-034 through HV-038.

## Phase 9: Librarian and writes

- [ ] Implement local SQLite metadata.
- [ ] Implement safe backup and location model.
- [ ] Implement edit-buffer send separately from memory write.
- [ ] Keep stored write internal-only behind build flag.
- [ ] Prepare and back up sacrificial slot.
- [ ] Pass HV-039 and HV-040.
- [ ] Only then expose stored write in production with explicit confirmation/audit.

## Release gate

- [ ] Exact tested Peak OS build recorded.
- [ ] macOS and Windows smoke suites pass.
- [ ] No enabled parameter lacks send/receive/semantic evidence.
- [ ] No raw MIDI constants in UI code.
- [ ] No default-enabled experimental/write command.
- [ ] Unknown SysEx bytes preserved.
- [ ] Signed/notarized distribution process documented.
