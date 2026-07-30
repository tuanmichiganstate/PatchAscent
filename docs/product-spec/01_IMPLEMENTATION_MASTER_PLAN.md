# Novation Peak Editor: verified implementation master plan

Prepared: 2026-07-30
Target: Novation Peak, current published firmware 2.1 feature set
Exact hardware OS build: pending capture from Settings > Version
Primary platforms: macOS and Windows
Primary connection: USB MIDI
Product form: standalone desktop application

## 1. Executive definition

Build a trustworthy, bidirectional software editor and patch librarian for the Novation Peak. The application must make menu-only parameters and modulation relationships easier to see than on the hardware while preserving the Peak as the sound engine and authority for actual synthesis behavior.

The editor is not a software emulation. It sends and receives MIDI, requests and preserves SysEx data, and manages local patch metadata. The highest-risk work is protocol correctness, state synchronization and destructive-write safety, not drawing knobs.

The initial release must support the currently published Peak firmware 2.1 feature set and record the exact OS build tested on the owner's unit. Novation's current downloads page exposes the 2.1 addendum as the current Peak feature addendum. Do not interpret this as proof that every unit reports the literal string "2.1"; capture the runtime value.

## 2. Authoritative product decisions

### 2.1 Included in version 1

- Standalone macOS and Windows application.
- One Peak connected at a time.
- Manual selection of MIDI input and output ports, with safe reconnection.
- Configurable Peak MIDI channel 1-16.
- Passive raw MIDI monitor and diagnostic export.
- Live parameter editing using verified CC, Peak CC-pair and NRPN codecs.
- Bidirectional updates from physical controls where Peak transmits them.
- Complete sound-oriented UI: oscillators, mixer, filter, envelopes, LFOs, voice, modulation, effects and arpeggiator.
- Complete 16-slot main modulation matrix after current enum codes are captured.
- Complete 4-slot FX modulation matrix after its mappings are captured.
- Undo/redo, A/B snapshots and dirty-state tracking.
- Opaque `.syx` capture, import and export.
- Edit-buffer request/load after the SysEx protocol is verified.
- Patch librarian and stored-memory write only after byte-safe round trip and sacrificial write tests.
- Local-first storage with no account requirement.

### 2.2 Explicit non-goals

- No VST3, AU or AAX in version 1.
- No audio capture, waveform display from audio, software oscillator or effects emulation.
- No firmware updater or bootloader commands.
- No Summit support in the Peak profile.
- No compatibility framework for older Peak firmware.
- No multi-device orchestration.
- No cloud, marketplace, telemetry, subscriptions or user accounts.
- No AI patch generation until protocol and librarian correctness are mature.
- No user-wavetable editor unless a verified protocol is later obtained.

## 3. Evidence hierarchy

Use this order when sources disagree:

1. Repeatable capture from the exact Peak unit and OS build under test.
2. Current official Novation guide and appendix.
3. Official firmware 2.1 addendum.
4. Official Peak 1.2 MIDI table for base mappings.
5. MIDI Association specifications for generic MIDI mechanics.
6. Third-party reverse engineering only as a test hypothesis.

Every mapping record must include source, page/section, evidence state, test IDs and the exact hardware build on which it was verified.

### 3.1 Meaning of "verified"

- `documented`: an official source states the mapping or feature.
- `receive verified`: Peak emitted the expected message when the physical/menu control changed.
- `send verified`: Peak responded correctly when the editor sent the message.
- `semantic verified`: raw values map correctly to labels/display units.
- `round-trip verified`: patch dump can be decoded and re-encoded byte-identically.
- `write verified`: a backed-up sacrificial slot was written, redumped and restored.

Do not collapse these into one boolean.

## 4. Verified device facts that shape the product

- Peak has extensive MIDI implementation; physical controls can transmit CC or NRPN and Peak can respond to received control data when `CC/NRPN` is set appropriately.
- The `CC/NRPN` global setting has Disabled, Receive, Transmit and Rec+Tran modes. The default is Rec+Tran.
- Patch selection is handled separately by Program/Bank Change under the `Bank/Patch` setting. CC/NRPN messages do not themselves contain patch data.
- Global Settings are not saved with individual patches. This mandates separate state models.
- The Backup menu can transmit Current, any one of Banks A-D, all four banks, Settings, or all banks plus Settings over USB or MIDI OUT.
- There are four banks of 128 patches.
- Patch Memory Protection can disable the Peak's patch-save function and should remain On during research.
- Peak provides 16 main modulation slots, each with two sources and one destination/depth, plus four FX modulation slots.
- Peak includes 60 factory wavetables, each consisting of five waveforms interpolated by Shape.
- All FX parameters are saved with a patch.
- Firmware 2.1 adds four spread modes, Pan Position, extra modulation destinations, Animate attack/release envelopes, extra LFO3/4 parameters, added FX destinations, Chorus/Flanger/Phlanger modes, additional delay modes/output/time modes, Noise as a modulation source, Arp Chance, Chord 2, Peak envelope Delay, Noise HPF, tuning-table NRPN response and corrected FM NRPN behavior.

## 5. Documentation conflicts and implementation consequences

The official documentation is sufficient to seed research but not safe to compile directly into a fully writable editor.

### 5.1 Peak CC-pair encoding is unspecified

The table labels parameters such as oscillator coarse/fine, mixer levels, filter frequency and LFO rates as `CC pair`, gives two CC numbers and usually a range of 0-255. It does not define whether the two CC values are nibbles, coarse/fine bytes, duplicated resolutions, a 7+1 bit arrangement or some other scheme. Conventional 14-bit CC pairing produces 0-16383 and must not be assumed.

Consequence: all CC-pair writes remain disabled until HV-007 and HV-008 establish a bijective encoder/decoder.

### 5.2 The current MIDI table is partly stale after firmware 2.1

The table still gives main modulation source range 0-16 and destination range 0-36, while the current appendix lists 25 sources and many more destinations. It also leaves Arp Type at 0-6 although Chord 2 is now listed.

Consequence: display labels may be built, but source/destination/type numeric codes remain null until captured.

### 5.3 Noise HPF range typo

The base/current MIDI table prints `0-0`; the official 2.1 addendum specifies 0-127, initial value 0.

Consequence: canonical range is provisionally 0-127 but remains hardware-tested before write enablement.

### 5.4 Arp Chance conflict

Official sources contain three formulations: addendum table 01-100, feature prose 10-100, and current guide heading 0-100 while its prose says 10-100.

Consequence: test raw 0, 1, 9, 10 and 100. Represent rejected/clamped behavior explicitly.

### 5.5 Pan mapping ambiguity

Feature text displays -64 to +63; the table maps NRPN 0:51 to 0-127 with a documented default of 0. The raw/display offset and default semantics cannot both be assumed from these statements.

Consequence: test 0, 63, 64 and 127 and record Peak's display.

### 5.6 Chorus mode naming conflict

The 2.1 table says `Chorus/Phaser/Flanger`; prose says `Chorus/Flanger/Phlanger`.

Consequence: raw codes 0,1,2 must be read from the display and stored with exact labels.

### 5.7 Boolean ranges conflict

The 1.2 PDF gives 0-1 for Arp KeySync and Animate Hold; the current online table shows 0-127.

Consequence: domain type is boolean, protocol codec must accept the actual values Peak emits and must not assume on=1 versus on=127.

### 5.8 Documentary defaults are not initialization data

Several table defaults are internally surprising or inconsistent. The editor must never initialize a patch by transmitting table defaults. Use an actual init-patch dump as the canonical initialization fixture.

## 6. Product requirements

### 6.1 Connection and device session

The application shall:

- Enumerate input and output ports independently.
- Show stable internal port IDs and human-readable names.
- Avoid auto-connecting solely by fuzzy name if more than one candidate exists.
- Persist the user's preferred port pair but validate it on each launch.
- Support MIDI channel 1-16.
- Show connection state: disconnected, connecting, connected-unidentified, identified, synchronizing, ready, degraded and error.
- Detect port loss and close resources cleanly.
- Reconnect safely without replaying stale queued edits.
- Record exact Peak OS build when captured.
- Expose CC/NRPN and Bank/Patch mode prerequisites in a connection checklist.

### 6.2 Live editing

- A user change updates local UI immediately, records history and enqueues one semantic parameter command.
- The scheduler serializes protocol sequences and coalesces superseded continuous values.
- Final hardware value must equal final UI value after a drag.
- Hardware-originated changes update UI without being retransmitted.
- Values outside verified raw ranges are impossible to send.
- Conflicted/unverified parameters are disabled in production mode and available only in Protocol Lab with warning and raw logging.

### 6.3 Patch synchronization

- Loading a local file does not automatically send it.
- Selecting a hardware patch does not silently destroy dirty local work.
- Editor state shows whether it is synchronized, locally modified, awaiting confirmation or based on an opaque dump.
- After a hardware patch selection, pending edits for the previous patch are cancelled.
- A verified edit-buffer request becomes the preferred authoritative snapshot.
- Without SysEx synchronization, the UI must not pretend it knows menu parameters that the Peak has not transmitted.

### 6.4 History and comparison

- Undo/redo is local semantic history, not MIDI byte history.
- Undo emits the appropriate verified parameter message.
- A/B snapshots store complete known PatchState and the opaque original dump when available.
- Compare view distinguishes known value changes, unknown fields and global settings.

### 6.5 Librarian

After SysEx gates pass, support:

- Opaque `.syx` import/export.
- Validation of F0/F7 framing, manufacturer/device identity when known, message type and expected length(s).
- Local metadata: user title, original patch name, category, tags, notes, favorite, source file, hash, import date and hardware location.
- Duplicate detection based on normalized sound payload only after name/header boundaries are verified; until then use whole-message SHA-256.
- Side-by-side parameter diff.
- Batch bank capture with progress, cancellation and resumability.
- Automatic backup before any stored-memory write.

## 7. Recommended repository architecture

```text
novation-peak-editor/
├── apps/
│   ├── desktop/                 # Tauri shell + React UI
│   └── peakctl/                 # Protocol laboratory CLI
├── crates/
│   ├── midi-transport/          # ports, callbacks, timestamps, hot-plug polling
│   ├── midi-messages/           # MIDI 1.0 raw/channel/system message types
│   ├── peak-protocol/           # CC, CC pair, NRPN, Program/Bank, SysEx framing
│   ├── peak-domain/             # parameter definitions, PatchState, SettingsState
│   ├── peak-sync/               # reconciliation, correlations, connection state machine
│   ├── peak-sysex/              # opaque dumps, later verified codecs
│   └── peak-library/            # local database and file management
├── packages/
│   ├── parameter-registry/      # generated TypeScript representation
│   ├── ui-components/           # controls, matrix rows, diagnostics
│   └── shared-types/            # generated IPC types where useful
├── protocol/
│   ├── parameter_registry.yaml
│   ├── enums.yaml
│   ├── evidence/
│   └── captures/                # gitignored private raw captures; fixtures curated separately
├── tests/
│   ├── unit/
│   ├── property/
│   ├── golden-midi/
│   ├── golden-sysex/
│   ├── fake-device/
│   └── hardware/
└── docs/
    ├── protocol/
    ├── decisions/
    └── release-validation/
```

Keep Tauri commands thin. Domain/protocol crates must be testable without launching the desktop UI.

## 8. Core domain types

Illustrative Rust model:

```rust
pub enum ChangeSource {
    UserInterface,
    PeakHardware,
    SysexLoad,
    ProgramSelection,
    Undo,
    Redo,
    Initialization,
    ProtocolLab,
}

pub enum ParameterScope { Patch, Global, RuntimeClock, Unknown }

pub enum EvidenceStatus {
    DocumentedUnverified,
    DocumentConflict,
    ReceiveVerified,
    SendVerified,
    SemanticVerified,
    SysexDecodeVerified,
    SysexRoundTripVerified,
    MemoryWriteVerified,
}

pub enum Binding {
    Cc { controller: u8 },
    CcPair { first: u8, second: u8, codec: CcPairCodec },
    Nrpn { msb: u8, lsb: u8, value_codec: NrpnValueCodec },
    Unmapped,
}

pub struct ParameterChange {
    pub event_id: u64,
    pub parameter_id: ParameterId,
    pub old_raw: Option<i32>,
    pub new_raw: i32,
    pub source: ChangeSource,
    pub request_hardware_send: bool,
    pub timestamp_micros: u64,
}
```

Patch values must allow known raw values, unknown values and conflicted decoded values. Do not use zero as a stand-in for unknown.

## 9. Parameter registry design

One machine-readable registry is the authority for both Rust and TypeScript. Generate target-language constants/types; never hand-copy controller numbers into components.

Each definition needs:

- stable semantic ID
- official label and aliases
- section and scope
- transport binding
- documentary raw range and display range
- display transform and formatter
- enum set ID
- firmware feature set
- device scope
- evidence status and source
- send/receive/SysEx verification booleans
- safety gate
- notes about conflicts

The included seed intentionally leaves all `live_write_enabled` fields false. The build should fail if production code references a controller not present in the generated registry.

## 10. MIDI transport and protocol implementation

### 10.1 Raw event model

Preserve every message before decoding:

```text
RawMidiEvent
  monotonic_timestamp
  wall_clock_timestamp
  port_id
  direction: in|out
  bytes: byte[]
  session_id
```

Derived events may reference the raw event ID. Never discard the raw bytes because a parser recognized a message.

### 10.2 CC codec

Implement ordinary 3-byte channel CC messages. Validate channel and 7-bit data. Unit-test all channels, controller/value boundaries and running-status handling if the backend surfaces raw running status.

### 10.3 Peak CC-pair codec

Create an interface with no production implementation initially:

```rust
pub trait CcPairCodec {
    fn encode(&self, raw: u16) -> Result<[(u8,u8); 2], ProtocolError>;
    fn ingest(&mut self, controller: u8, value: u8, timestamp: u64)
        -> Option<u16>;
}
```

The implementation must be derived from HV-007/HV-008. It must specify order tolerance, pair timeout, whether either member alone can update coarse state, and how Peak transmits adjacent 8-bit values.

### 10.4 NRPN codec

Generic MIDI uses CC99 for NRPN MSB, CC98 for NRPN LSB and CC6 for Data Entry MSB, optionally CC38 for Data Entry LSB. That is a candidate base sequence, not yet proof of Peak's preferred behavior.

Implement:

- encoder strategy selectable per device profile
- parser state per port and MIDI channel
- selector state for NRPN versus RPN
- timestamps and expiry
- support for data entry MSB and optional LSB
- data increment/decrement recognition for diagnostics
- optional null-selector termination, disabled until tested

Outgoing sequences must hold an exclusive scheduler lock from first selector message through final value/termination message.

### 10.5 Program and Bank Change

Do not hard-code bank values from generic MIDI. Capture Peak output across banks A-D. Model patch location semantically as bank A-D and program 1-128; map to wire bytes through a verified codec.

Program/Bank Change observation is a synchronization event, not proof of complete patch parameters.

### 10.6 System Exclusive

Initial SysEx implementation is framing and storage only:

- assemble F0 through F7 across backend callbacks
- reject nested F0 or premature termination with diagnostics
- impose configurable maximum length, high enough for full official bank dumps
- store exact bytes and SHA-256
- identify universal versus manufacturer-specific messages
- do not mutate or "repair" captures silently

A parsed view must retain the original immutable byte buffer.

### 10.7 Output scheduler

One scheduler owns each output port. Required command classes:

- atomic single message
- atomic multi-message sequence
- continuous coalescible parameter update
- non-coalescible command
- SysEx transfer with progress

Rules:

- Never interleave NRPN sequences.
- Coalesce pending values for the same continuous parameter, retaining the latest.
- Never coalesce Program Change, button edge, SysEx or explicit audit commands.
- Cancel stale edits on patch change/disconnect.
- Start with conservative pacing; replace with tested USB/DIN profiles after soak tests.
- Expose queue depth, coalesced count, sent count, error count and oldest-item age.

## 11. Input parsing and synchronization

### 11.1 Parser pipeline

```text
backend callback
  -> timestamp/raw log
  -> message framer
  -> channel/system decoder
  -> Peak protocol decoder
  -> semantic event
  -> synchronization reducer
  -> domain state
  -> UI subscription
```

No React component listens directly to raw MIDI.

### 11.2 Correlation model

For an outbound semantic change, create a correlation record with expected wire representation and expiry. An inbound match marks hardware confirmation. It does not trigger a resend. A different inbound value is a hardware-originated change and updates state.

### 11.3 Initialization

Preferred final sequence:

1. Enumerate/select ports.
2. Open input before output to avoid missing replies.
3. Run passive readiness checks.
4. Optionally send Universal Device Inquiry after verified.
5. Record connection profile and exact OS build if obtainable.
6. Request current edit buffer after verified.
7. Decode known fields, preserve opaque dump, atomically set baseline.
8. Enter Ready.

Before edit-buffer requests are verified, use a degraded live mode that updates only parameters observed or explicitly changed. Clearly label incomplete state.

## 12. User interface information architecture

### 12.1 Connection screen

Show input/output ports, MIDI channel, detected device identity/build, CC/NRPN prerequisite, Bank/Patch prerequisite, sync status and a link to diagnostics. Include a test button that performs passive monitoring first; no automatic broad write test.

### 12.2 Sound page

- Three oscillator columns with common controls visibly separated.
- Mixer section.
- Filter section.
- Amplifier/voice section including unison, spread mode/amount and pan.
- Clear distinction between raw value and musical display.
- Copy/paste oscillator section after complete state support.

### 12.3 Envelopes and LFO page

- Three DAHDSR envelopes, including Delay, Hold and Repeat where mappings are verified.
- Animate 1/2 attack/release.
- LFO 1/2 per-voice controls.
- LFO 3/4 global controls and FX relevance.
- Unknown mapping controls visibly disabled in developer builds rather than omitted from the model.

### 12.4 Modulation page

Make the 16-slot matrix the flagship view. Each row:

```text
Slot | Source A | Source B | Depth | Destination | status
```

Provide searchable menus, filters for used/unused slots, duplicate-route warnings, destination grouping and raw-code display in diagnostics. Do not assign numeric codes by array index until captured.

### 12.5 Effects page

- Distortion, Chorus/Flanger/Phlanger, Delay and Reverb.
- Routing visualization for parallel and six serial orders.
- Delay mode, output and time mode.
- Four-row FX modulation matrix.
- All effects belong to PatchState.

### 12.6 Arpeggiator page

- On, latch, key sync, rate/sync, type, rhythm, octaves, gate, swing and chance.
- Chord and Chord 2 semantic explanation.
- External MIDI clock status separate from patch controls.
- Raw conflict diagnostics for Chance and Type until verified.

### 12.7 Patch and librarian page

- Current patch identity and sync state.
- Local save versus send to edit buffer versus write to Peak memory are separate buttons.
- Destination shown as bank letter plus 1-based program number.
- Hardware write is visually and procedurally distinct.
- Backup status and verification result shown before completion.

### 12.8 Diagnostics drawer

Expose:

- port IDs/names and backend
- channel and modes
- exact OS build
- raw event monitor with filters
- decoded message and parameter ID
- queue metrics
- last SysEx messages and hashes
- dropped/invalid message count
- parameter evidence status
- exportable support bundle

## 13. SysEx research and codec strategy

### 13.1 Opaque-first rule

A valid captured dump is first-class data even when zero payload fields are decoded. Import/export and hashing can ship before patch editing through SysEx.

### 13.2 Differential reverse engineering

Use a known baseline, change exactly one item, dump again and compare. For each parameter capture minimum, adjacent minimum, center-adjacent, center, adjacent maximum and maximum where safe. Restore baseline between tests.

Record:

- changed offsets
- bit masks
- endianness
- duplicate/check bytes
- whether patch name/location/header is included
- whether dynamic bytes change between identical dumps

### 13.3 Decoder/encoder model

Represent the dump as:

```text
OpaqueDump
  original bytes
  validated framing
  header view
  payload ranges
  decoded known fields with byte provenance
  unknown ranges
  dynamic/checksum ranges
```

Encoding begins from original bytes and patches only verified fields. Never reconstruct the full dump from defaults.

### 13.4 Round-trip gates

- Unchanged decode/encode must be byte-identical.
- Editing one field must change only expected offsets plus verified checksum/dynamic bytes.
- Peak must accept edit-buffer send and redump equivalent state.
- Unknown bytes must survive.
- Multiple factory/user patches must pass, not only one fixture.

### 13.5 Third-party prior art

KnobKraft Orm contains an alpha Summit/Peak adaptation with candidate Novation/Peak IDs, dump requests and a name offset. It is useful only to form experiments. The repository is AGPL by default with a separate commercial MIT option. Do not copy code into an incompatibly licensed project, and do not promote those constants without clean-room Peak captures.

## 14. Safe write workflow

Separate four operations:

1. Save local project/file: no hardware operation.
2. Send parameter changes or patch to temporary edit buffer: non-persistent but audible.
3. Select stored hardware patch: Program/Bank Change.
4. Write stored patch/global settings: destructive/persistent.

Stored write requirements:

- exact identified Peak and tested build
- synchronized current state
- selected bank/program destination
- destination backup and hash
- Patch Protect state explicitly addressed
- typed or two-step confirmation showing destination and existing patch name
- one serialized write operation
- response/timeout handling
- automatic redump and semantic/byte verification
- rollback option using backup
- immutable audit record

Never write because a user single-clicked a librarian row.

## 15. Persistence

Begin with local JSON/session logs and `.syx` files. Add SQLite for librarian metadata when Milestone 4 starts.

Suggested tables:

- `patch_objects`: IDs, whole-message hash, payload hash after verified, original bytes path.
- `patch_versions`: parent, timestamp, known-state JSON, source.
- `library_metadata`: tags, notes, favorite, category override.
- `hardware_locations`: bank, program, last verified hash/time/build.
- `protocol_evidence`: parameter, test, build, capture files, verification state.
- `write_audit`: operation, destination, backup hash, sent hash, result, redump hash.

Do not store large SysEx blobs in frontend localStorage.

## 16. Testing strategy

### 16.1 Unit tests

- channel message encode/decode
- NRPN parser state transitions
- scheduler sequence locking
- coalescing semantics
- parameter range validation
- display transforms
- state reducer provenance and dirty flags
- SysEx framing errors
- file/hash behavior

### 16.2 Property tests

- verified codec `decode(encode(x)) == x` across complete raw ranges
- parser never panics for arbitrary byte streams
- unknown SysEx bytes remain unchanged
- scheduler preserves non-coalescible order
- state events remain deterministic

### 16.3 Golden tests

Use captured byte sequences with manifest and exact hardware build. Commit only non-sensitive curated fixtures. Preserve original hashes.

### 16.4 Fake Peak

Build a deterministic fake MIDI device service that:

- accepts verified CC/NRPN
- can echo, delay, drop or reorder messages
- emits Program Changes
- returns fixture SysEx dumps
- disconnects on command
- detects interleaved NRPN sequences

This supports CI but never replaces hardware gates.

### 16.5 Hardware tests

Use `tests/hardware_verification_matrix.csv`. Every pass attaches raw logs/captures, environment manifest, software commit and tester notes. Re-run release-critical tests on macOS and Windows.

## 17. Security and reliability

- Validate all file paths and imported file sizes.
- Never execute content from patch metadata.
- Treat MIDI input as untrusted byte streams.
- Bound queues and SysEx buffers.
- Avoid logging user filesystem paths in exported diagnostics unless consented.
- No network requirement for editing.
- Tauri command allowlist: expose semantic operations only, not arbitrary file or raw-device access from the webview.
- Crash recovery should preserve local dirty state but must not automatically replay hardware writes.

## 18. CI and release engineering

Required checks:

- Rust format, clippy and tests.
- TypeScript lint, typecheck and tests.
- Registry schema validation and duplicate-binding validation.
- Generated Rust/TypeScript artifacts are reproducible and clean.
- Fake-device integration tests.
- Dependency/license scan.
- macOS and Windows build smoke tests.
- Release manifest includes commit, app version, supported Peak feature set and exact hardware build(s) tested.

Code signing/notarization can be added after internal alpha but must precede broad distribution.

## 19. Milestones and acceptance gates

### Milestone 0: Protocol laboratory

Deliver `peakctl` and a minimal Tauri diagnostics shell.

Scope:

- port enumeration/open/close
- raw monitor and session logging
- ordinary CC codec
- candidate NRPN codec behind feature flag
- output scheduler
- SysEx framing/capture
- evidence manifest and hashes

Gate:

- HV-001 through HV-014 pass.
- CC-pair codec remains disabled unless HV-008 passes.
- No full graphical synth editor yet.

### Milestone 1: Connected core editor

Scope:

- DeviceSession state machine.
- Parameter registry generator.
- PatchState/GlobalSettings separation.
- Oscillator, mixer, filter and basic envelope controls for verified mappings.
- Hardware-to-UI synchronization.
- Undo/redo and dirty state.

Gate:

- No feedback loops in soak test.
- Final state matches hardware after rapid drags.
- Unverified controls are disabled and explain why.

### Milestone 2: Full documented live control surface

Scope:

- All verified CC/CC-pair/NRPN parameters.
- Firmware 2.1 controls.
- Complete current enum code captures.
- Main and FX modulation matrices.
- Effects and arpeggiator.
- Diagnostics and evidence browser.

Gate:

- 100% of enabled controls have send, receive and semantic verification records.
- No controller constants outside generated registry.

### Milestone 3: Edit-buffer SysEx synchronization

Scope:

- verified device inquiry if useful
- edit-buffer request
- dump validator
- opaque dump baseline
- incremental verified decoder
- byte-identical encoder
- send to edit buffer

Gate:

- HV-015 through HV-038 pass as applicable.
- Multiple varied patches round-trip.
- Unknown bytes preserved.

### Milestone 4: Librarian and safe stored writes

Scope:

- SQLite library
- individual patch and bank capture
- import/export/search/tags/compare
- backup before write
- sacrificial stored-write verification
- rollback and audit

Gate:

- HV-039 and HV-040 pass.
- Destructive action cannot be triggered accidentally.

### Milestone 5: Creative workflow

Only after protocol reliability:

- section copy/paste
- A/B compare
- constrained randomization using verified ranges/enums
- mutation amount
- morphing between compatible known parameter states
- macros and patch lineage

Do not mutate opaque/unknown bytes.

### Milestone 6: Distribution hardening

- signed/notarized installers
- Windows regression suite
- crash reporting only if opt-in and privacy-reviewed
- user documentation
- exportable support bundle
- release validation on recorded Peak OS build(s)

## 20. Epics and ordered work packages

### Epic A: Repository and governance

- Establish monorepo, formatting and CI.
- Copy bundle registry into project as source data.
- Add schema and validation gates.
- Add protocol evidence folder and template.
- Add feature flags: `protocol_lab`, `cc_pair_experimental`, `sysex_requests_experimental`, `hardware_writes_internal`.

Acceptance: default build contains no experimental or persistent hardware write path.

### Epic B: MIDI transport

- Implement backends through `midir` abstraction.
- Enumerate and identify ports.
- Timestamp callbacks.
- Open input before output.
- Bounded raw-event channel.
- Disconnect handling and port rescans.

Acceptance: HV-003/HV-004.

### Epic C: Message codecs

- Raw MIDI types.
- CC encode/decode.
- NRPN parser/encoder candidate.
- Program/Bank event parser.
- SysEx framer.
- Fuzz/property tests.

Acceptance: byte-exact golden tests plus HV-005/HV-006/HV-009/HV-010.

### Epic D: Scheduler

- Atomic sequences.
- Parameter coalescing.
- cancellation and queue metrics.
- pacing profiles.
- deterministic fake-device tests.

Acceptance: HV-013/HV-014 and no stale replay after disconnect.

### Epic E: Domain and synchronization

- generated parameter types
- state partitions
- change provenance
- outbound correlations
- undo/redo
- patch-change transition

Acceptance: semantic reducer tests and HV-012/HV-022/HV-023.

### Epic F: Editor UI

- connection page
- reusable control primitives
- sound pages
- matrices
- diagnostics
- accessibility and keyboard navigation

Acceptance: every visible enabled control resolves to one registry definition and evidence state.

### Epic G: SysEx and librarian

- opaque capture/import/export
- differential tooling
- verified decoder/encoder
- local DB
- writes and rollback

Acceptance: round-trip and sacrificial-write gates.

## 21. Definition of done for any parameter

A parameter is production-ready only when:

- stable ID and scope are defined
- official source is cited
- wire binding is known
- raw range is verified
- display transform or enum code is verified
- send and receive tests pass on exact Peak build
- UI control handles unknown/disconnected states
- undo/redo works
- no feedback loop occurs
- unit/golden tests exist
- evidence record links captures
- if represented in SysEx, field mapping round-trips

## 22. Risk register

| Risk | Impact | Mitigation |
|---|---|---|
| Official table conflicts/staleness | Wrong controls or values | Evidence states, disabled mappings, hardware capture |
| Incorrect CC-pair assumption | Widespread wrong high-resolution parameters | Dedicated codec research before enablement |
| NRPN interleaving | Wrong parameter changes | One serialized sequence scheduler |
| MIDI echo loop | Flooding and unstable values | provenance and outbound correlations |
| Incomplete initial state | UI lies about patch | degraded mode, edit-buffer dump as final authority |
| Unknown SysEx bytes lost | Corrupted patches | original-byte patching, byte-identical gates |
| Wrong bank write | User data loss | backup, destination confirmation, sacrificial tests, redump |
| Port ownership/contention | Connection failure with DAW | explicit errors, selectable ports, recovery tests |
| USB/DIN behavioral differences | Inconsistent reliability | separate tested pacing profiles |
| Third-party license contamination | Distribution/legal risk | clean-room implementation, no source copying |
| Tauri webview overreach | Security/file/device exposure | narrow semantic IPC allowlist |
| Firmware/build drift | Regression | runtime build capture and release matrix |

## 23. Decision log

Accepted:

- Standalone first, plugin later only if justified.
- Tauri/React/TypeScript/Rust baseline.
- Peak only, current 2.1 feature set.
- Evidence-driven parameter registry.
- Opaque-first SysEx.
- All hardware memory writes disabled until verified.
- Global settings outside PatchState.

Pending evidence:

- exact Peak OS build
- CC-pair codec
- Peak-specific NRPN sequence details
- bank values
- SysEx request/dump/write protocol
- current enum codes and several firmware 2.1 parameter mappings

## 24. First implementation increment, exact tasks

1. Create workspace and CI.
2. Import and validate registry seed.
3. Implement `RawMidiEvent` and JSONL log format.
4. Implement port listing and open/close.
5. Implement raw monitor with hex, decimal and interpreted channel message.
6. Implement CC send command restricted to allowlisted test parameter CC79.
7. Implement candidate NRPN sender restricted to allowlisted test parameter 0:14 behind experimental flag.
8. Implement sequence scheduler and test interleaving.
9. Implement SysEx framer/capture only, no manufacturer request.
10. Produce first hardware capture session and update evidence.
11. Implement CC-pair analysis tool that plots/tables observed first/second CC values, without sending pairs yet.
12. After tests pass, expose the first semantic controls in desktop UI: Filter Resonance and Oscillator 1 Wave.

## 25. Files the owner can provide

The highest-value next inputs are:

- clear photo/screenshot of Settings > Version
- raw MIDI log for Filter Resonance, Filter Frequency and Oscillator 1 Wave
- Current patch SysEx backup, repeated twice unchanged
- one stored patch dump or full Bank A backup
- Settings backup
- later, controlled one-parameter differential dumps

Follow `03_HARDWARE_CAPTURE_GUIDE.md` and include the manifest. No stored write is required for early phases.

## 26. Source entry points

- Peak downloads and official documents: https://downloads.novationmusic.com/novation/synthesisers/peak
- Current Peak guide: https://userguides.novationmusic.com/hc/en-gb/articles/25494693870354-Novation-Peak-in-detail
- Current appendix/MIDI table: https://userguides.novationmusic.com/hc/en-gb/articles/25494651392146-Peak-appendix
- Firmware 2.1 addendum: https://fael-downloads-prod.focusrite.com/customer/prod/downloads/summit_peak_2.1_firmware_update_addendum_v1_english_en.pdf
- Base Peak MIDI table: https://fael-downloads-prod.focusrite.com/customer/prod/s3fs-public/downloads/Peak%201.2%20Manual%20-%20English.pdf
- Components update guidance: https://support.novationmusic.com/hc/en-gb/articles/360002211360-Updating-firmware-using-Novation-Components
- MIDI Association CC/NRPN reference: https://midi.org/midi-1-0-control-change-messages
- Tauri: https://v2.tauri.app/
- `midir`: https://github.com/Boddlnagg/midir
- Third-party hypothesis only, KnobKraft: https://github.com/christofmuc/KnobKraft-orm
