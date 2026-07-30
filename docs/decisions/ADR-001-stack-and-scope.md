# ADR-001: Desktop stack and version-1 scope

Status: Accepted

## Decision

Use Tauri 2 as the desktop shell, React and TypeScript for the user interface, Rust for device/protocol/state services, and `midir` as the initial cross-platform MIDI backend abstraction.

Official Tauri documentation describes a cross-platform architecture combining a web frontend with Rust application logic. The `midir` project supports CoreMIDI on macOS, WinMM/WinRT on Windows and full SysEx transfer. Sources:

- https://v2.tauri.app/
- https://github.com/Boddlnagg/midir

## Rationale

The product needs a rich, data-driven editor and native MIDI/file access. Rust is appropriate for byte-level parsing, deterministic scheduling and safety boundaries. React is appropriate for a large parameter UI and matrix editors. Tauri avoids making browser Web MIDI support the foundation of the shipped application.

## Version-1 boundaries

Included:

- macOS and Windows desktop application.
- One connected Peak.
- USB MIDI as the primary certified path; DIN MIDI may be tested later.
- Live CC/NRPN editing after hardware verification.
- Bidirectional panel synchronization.
- Complete 16-slot main modulation matrix and 4-slot FX matrix once codes are verified.
- Opaque SysEx capture/import/export before decoded librarian work.
- Safe edit-buffer loading and, later, explicitly confirmed memory writes.

Excluded:

- VST3/AU/AAX.
- Audio recording or Peak DSP emulation.
- Firmware updater.
- Summit and other synthesizers.
- Older firmware compatibility UI.
- Cloud sync, accounts, marketplace or telemetry.
- AI patch generation.
- Automatic destructive writes.

## Consequence

Keep domain and protocol crates independent of Tauri and React so a future plugin host can reuse them without redesigning the MIDI model.
