# ADR-002: Protocol evidence and write safety

Status: Accepted

## Decision

All protocol behavior is governed by explicit evidence states and operation safety classes.

Evidence states:

1. `documented_unverified`
2. `document_conflict`
3. `hardware_receive_verified`
4. `hardware_send_verified`
5. `semantic_transform_verified`
6. `sysex_decode_verified`
7. `sysex_encode_roundtrip_verified`
8. `hardware_memory_write_verified`

Safety classes:

- S0: passive monitoring and local file reading.
- S1: non-destructive live CC/NRPN change to current edit state.
- S2: SysEx request or send to temporary edit buffer.
- S3: stored patch or global settings write.
- S4: firmware/bootloader operation, permanently out of scope.

## Rules

- S0 may be enabled first.
- S1 requires receive/send verification for the exact codec family.
- S2 requires valid framing, opaque round trip, known target semantics and rollback.
- S3 requires a complete backup, explicit destination confirmation, memory-protection workflow, post-write redump and byte verification.
- S4 must not exist in this codebase.

All operations must produce an append-only audit event with timestamp, target, raw bytes or capture hash, result and user confirmation where applicable.
