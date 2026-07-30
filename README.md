# PatchAscent

PatchAscent is a standalone macOS and Windows editor and protocol laboratory for one physical Novation Peak synthesizer.

The project is deliberately at **Milestone 0: Protocol Laboratory**. Its first executable is `peakctl`: a read-first tool for port discovery, byte-preserving monitoring, ordinary MIDI decoding, approved CC/experimental NRPN tests, opaque SysEx capture, SHA-256 hashing, and timestamped session logs. The graphical app is currently a diagnostics shell—not a complete synth editor.

## Safety boundary

- No stored patch, settings, firmware, or bootloader write command exists.
- All imported registry definitions have live writes disabled.
- The only initial CC test target is Filter Resonance (CC 79), with an explicit runtime acknowledgement.
- The candidate NRPN sender is absent from default builds and, when compiled experimentally, is restricted to Oscillator 1 Wave (0:14).
- Peak CC-pair encoding is unknown and has no production encoder.
- SysEx is framed and stored opaquely; no manufacturer-specific request or write is enabled.

See [the coding directive](docs/product-spec/CODING_AGENT_START_HERE.md), [the master plan](docs/product-spec/01_IMPLEMENTATION_MASTER_PLAN.md), and [the hardware capture guide](docs/product-spec/03_HARDWARE_CAPTURE_GUIDE.md).

## Workspace

```text
apps/
  desktop/       Tauri 2 + React diagnostics shell
  peakctl/       read-first protocol lab CLI
crates/
  fake-peak/     deterministic CI simulator (never hardware evidence)
  midi-messages/ raw and decoded MIDI 1.0 messages
  midi-transport/midir-backed ports and sessions
  peak-domain/   registry and separated state models
  peak-protocol/ CC/NRPN protocol logic and safety gates
  peak-sync/     output queue, coalescing, and correlation
  peak-sysex/    opaque SysEx framing and hashing
  peak-library/  future librarian boundary (no writes)
packages/
  parameter-registry/ generated TypeScript registry
protocol/        canonical evidence-governed source data
```

## Prerequisites

- Node.js 22 or newer
- npm 10 or newer
- Current stable Rust toolchain
- macOS or Windows for the certified desktop targets
- A Peak is not required for unit tests

## Local checks

```bash
npm install
npm run registry:generate
npm run quality
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The Python source validator from the engineering bundle additionally needs:

```bash
python3 -m venv .venv
.venv/bin/pip install -r requirements-dev.txt
.venv/bin/python scripts/validate_registry.py protocol/parameter_registry.yaml
```

## Running the protocol lab

Passive commands:

```bash
cargo run -p peakctl -- ports
cargo run -p peakctl -- monitor --input <PORT_ID> --log-dir sessions
cargo run -p peakctl -- capture-sysex --input <PORT_ID> --output sessions/current.syx
```

The CLI prints exact safety requirements before any live edit. Run `peakctl <command> --help` for the acknowledgement flags and restrictions. Hardware verification results belong in `protocol/evidence/`; documentation alone must never promote a mapping.

## Status

Software-only verification can establish parser, queue, registry, and file behavior. Hardware gates HV-001 through HV-014 still require the owner's exact Peak and cannot be marked passed by CI or a fake device.
