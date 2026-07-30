# PatchAscent implementation status

Updated 2026-07-30.

## Implemented software scope

- Tauri 2 and React diagnostics shell with passive port discovery and a byte-preserving raw monitor.
- `peakctl` port listing, raw monitoring, timestamped JSONL logs, opaque SysEx capture/hashing, and receive-only CC-pair analysis.
- Ordinary MIDI channel decoding and the allowlisted Filter Resonance CC 79 live-edit test.
- Candidate Oscillator 1 Wave NRPN 0:14 sender behind an off-by-default Cargo feature.
- One serialized output scheduler with atomic sequences, coalescing, cancellation, metrics, and outbound correlation.
- Separate patch, global-settings, device-session, librarian, and history state models with provenance and unknown values.
- Canonical 251-record registry validation and reproducible TypeScript generation.
- Deterministic fake Peak integration tests for echo, delay, drop, reorder, program change, fixture SysEx, disconnect, and NRPN interleaving.
- Rust, TypeScript, registry, dependency, license, and macOS/Windows smoke-test CI definitions.

## Evidence boundary

All hardware verification gates remain pending. A fake device and software tests do not satisfy HV-001 through HV-014, establish the exact Peak OS build, or promote any mapping’s evidence status.

The full synth editor, Peak-specific SysEx decoder/request protocol, CC-pair encoder, stored-memory writes, settings writes, and firmware operations remain absent by design.

## Next physical inputs

Follow `docs/product-spec/03_HARDWARE_CAPTURE_GUIDE.md` and begin with:

1. Exact Settings > Version evidence.
2. Passive Filter Resonance, Filter Frequency, and Oscillator 1 Wave captures.
3. Two unchanged Current patch SysEx captures with manifests and hashes.

Store evidence records under `protocol/evidence/`; do not change registry gates without the matching raw artifact and hardware test ID.
