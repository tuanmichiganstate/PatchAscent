# Novation Peak Editor engineering bundle

Prepared: 2026-07-30

This bundle is a coding-ready baseline for a standalone Novation Peak editor. It targets the feature set published in Novation's Summit and Peak firmware 2.1 addendum. The exact OS build installed on the user's hardware is deliberately recorded as **pending capture**, because "latest" in Components and "2.1 feature set" do not prove an exact runtime build.

## Start here

1. Read `CODING_AGENT_START_HERE.md`.
2. Treat `01_IMPLEMENTATION_MASTER_PLAN.md` as the product and engineering specification.
3. Import `protocol/parameter_registry_seed.yaml`, but keep every write gate disabled until the corresponding hardware test passes.
4. Use `tests/hardware_verification_matrix.csv` as the integration-test ledger.
5. Use `03_HARDWARE_CAPTURE_GUIDE.md` to collect the missing Peak traffic.

## Non-negotiable safety rules

- Do not guess undocumented MIDI mappings.
- Do not interpret Peak "CC pair" entries as conventional 14-bit CC pairs without evidence.
- Do not send any SysEx memory-write command until an edit-buffer round trip is byte-safe and a sacrificial destination has been backed up.
- Preserve every unknown SysEx byte.
- Keep Patch Memory Protection On during research, except for a tightly controlled sacrificial write test.
- Never let a UI component construct raw MIDI bytes.
- Keep global Settings separate from PatchState.

## Bundle map

- `01_IMPLEMENTATION_MASTER_PLAN.md`: comprehensive product, architecture, protocol, roadmap and acceptance specification.
- `02_VERIFIED_EVIDENCE_AND_CONFLICTS.md`: source hierarchy, verified facts and documentation discrepancies.
- `03_HARDWARE_CAPTURE_GUIDE.md`: exactly what the owner should capture and upload.
- `04_AGENT_EXECUTION_CHECKLIST.md`: ordered build instructions for the coding agent.
- `protocol/official_parameter_table_raw.csv`: extracted official base and 2.1 mappings, preserving documentary ranges/defaults and warnings.
- `protocol/parameter_registry_seed.yaml`: conservative machine-readable seed, with all live writes disabled.
- `protocol/enums_and_lists.yaml`: official current labels; unknown numeric codes are null.
- `protocol/open_protocol_questions.csv`: unresolved questions and verification methods.
- `protocol/reverse_engineered_hypotheses.yaml`: quarantined third-party hypotheses, not production constants.
- `tests/hardware_verification_matrix.csv`: hardware tests and pass criteria.
- `schemas/`: data validation schemas.
- `architecture/`: architecture decisions.
- `scripts/validate_registry.py`: basic registry consistency check.

## Official source entry points

- Peak downloads: https://downloads.novationmusic.com/novation/synthesisers/peak
- Current Peak guide: https://userguides.novationmusic.com/hc/en-gb/articles/25494693870354-Novation-Peak-in-detail
- Current Peak appendix and MIDI table: https://userguides.novationmusic.com/hc/en-gb/articles/25494651392146-Peak-appendix
- Firmware 2.1 addendum: https://fael-downloads-prod.focusrite.com/customer/prod/downloads/summit_peak_2.1_firmware_update_addendum_v1_english_en.pdf
- Components update guidance: https://support.novationmusic.com/hc/en-gb/articles/360002211360-Updating-firmware-using-Novation-Components
