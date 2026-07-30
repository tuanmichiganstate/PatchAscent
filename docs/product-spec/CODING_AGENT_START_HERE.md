# Coding agent directive

You are implementing a standalone desktop editor for one physical Novation Peak synthesizer. Read every file in this bundle before writing production code.

## Product decision already made

Build version 1 as a standalone macOS and Windows application. Use Tauri 2, React, TypeScript and Rust, with MIDI I/O behind a Rust abstraction. Do not build VST3, AU, an audio engine, cloud accounts, firmware updating, Summit support, multi-device operation or backward-compatibility branches for older Peak firmware.

## Governing rule

Evidence outranks convenience. A documented parameter is not automatically a verified protocol parameter. Every mapping must move through these states:

`documented -> send verified -> receive verified -> semantic/display verified -> dump round-trip verified`

Do not silently promote a mapping. Record evidence artifacts and the test ID that allowed the transition.

## First deliverable

The first executable is `peakctl`, a read-first protocol laboratory. It must enumerate ports, monitor bytes, decode standard channel messages without losing raw bytes, send explicitly approved CC/NRPN test messages, capture SysEx, hash captures and export a timestamped session log.

Do not begin the complete synth UI until tests HV-001 through HV-014 pass. Do not begin SysEx decoding until HV-015 through HV-017 pass. Do not implement any hardware memory write until HV-036 through HV-040 pass in sequence.

## Mandatory architecture

- UI emits semantic intents, never MIDI bytes.
- Domain state is independent of React components.
- Protocol codecs are pure and unit-tested.
- All outgoing MIDI runs through one serialized scheduler.
- All incoming messages retain timestamp, port, channel and raw bytes.
- Patch state, global settings, device/session state and librarian metadata are separate models.
- Unknown or conflicted values remain representable as unknown; do not coerce them to defaults.
- Every parameter carries provenance and verification status.

## Immediate command sequence

1. Scaffold monorepo and CI.
2. Implement protocol-domain types and raw monitor.
3. Load the registry seed and validate it.
4. Implement CC codec.
5. Implement a generic NRPN candidate codec behind a feature flag.
6. Implement scheduler, coalescing and sequence locking.
7. Run the P0 hardware tests and update evidence records.
8. Only then create the first connected editor page.

Use `04_AGENT_EXECUTION_CHECKLIST.md` for the granular order and `01_IMPLEMENTATION_MASTER_PLAN.md` for all acceptance criteria.
