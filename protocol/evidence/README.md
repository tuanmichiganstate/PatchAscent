# Protocol evidence

Each mapping promotion needs an evidence record based on a test in `tests/hardware/hardware_verification_matrix.csv`. Store capture hashes and relative artifact references here. Do not commit private paths or broad project folders.

The evidence ladder is dimensional, not one boolean:

1. documented
2. hardware receive verified
3. hardware send verified
4. semantic/display verified
5. SysEx decode verified
6. SysEx round-trip verified
7. memory write verified

CI and fake-device tests cannot promote a hardware verification state.
