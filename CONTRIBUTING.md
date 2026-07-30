# Contributing to PatchAscent

The protocol evidence rules in `docs/product-spec/` govern every change.

## Parameter changes

A parameter is not production-ready because it appears in an official table. Update the canonical registry, attach the relevant hardware test ID and evidence artifact, and promote each verification dimension independently. Never infer enum codes from label order or interpret Peak CC pairs as conventional 14-bit pairs without a passing capture.

## Safety classes

- S0: passive monitoring and local file reading
- S1: verified non-persistent CC/NRPN edit-buffer changes
- S2: verified SysEx request or temporary edit-buffer transfer
- S3: backed-up, explicitly confirmed stored-memory write
- S4: firmware/bootloader operations; permanently out of scope

Default builds must contain only the paths whose evidence gates have passed. No frontend component may construct raw MIDI bytes.

## Evidence

Use `protocol/evidence/TEMPLATE.json` and reference a test from `tests/hardware/hardware_verification_matrix.csv`. Private raw captures remain gitignored until deliberately curated.
