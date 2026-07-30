# Verified evidence, source hierarchy and conflicts

This document separates facts supported by official sources from matters requiring hardware capture.

## Official source set

1. Peak downloads page, currently listing the Summit and Peak 2.1 firmware addendum and current Peak guide: https://downloads.novationmusic.com/novation/synthesisers/peak
2. Current Peak guide: https://userguides.novationmusic.com/hc/en-gb/articles/25494693870354-Novation-Peak-in-detail
3. Current Peak appendix and MIDI table: https://userguides.novationmusic.com/hc/en-gb/articles/25494651392146-Peak-appendix
4. Summit and Peak Firmware Update Version 2.1 Addendum: https://fael-downloads-prod.focusrite.com/customer/prod/downloads/summit_peak_2.1_firmware_update_addendum_v1_english_en.pdf
5. Peak 1.2 manual, pages 40-43 for the base MIDI table: https://fael-downloads-prod.focusrite.com/customer/prod/s3fs-public/downloads/Peak%201.2%20Manual%20-%20English.pdf
6. MIDI Association generic MIDI references: https://midi.org/midi-1-0-control-change-messages and https://midi.org/about-midi-part-3midi-messages

## Officially supported facts

### Device and state separation

- Peak control data can transmit and receive through CC/NRPN according to a global four-mode setting.
- Program/Bank Change is separate from CC/NRPN and is controlled by its own four-mode setting.
- Settings are global and not stored with each patch.
- PatchCue is a global Setting.
- All FX parameters are patch data.
- Backup choices distinguish Current patch, individual/full banks and Settings.
- Four banks contain 128 patches each.
- Patch Memory Protection can disable saving to patch memory.

### Modulation capacity

- Main matrix: 16 slots, two sources per slot.
- FX matrix: four slots, two sources per slot.
- Current official labels are included in `protocol/enums_and_lists.yaml` without guessed codes.

### Firmware 2.1 mappings explicitly listed

| Parameter | NRPN | Documentary range |
|---|---:|---:|
| Pan Position | 0:51 | 0-127 |
| Pan/Spread Type | 0:52 | 0-3 |
| Spread Amount | 0:5 | 0-127 |
| Chorus mode | 0:115 | 0-2 |
| Lo-Fi Delay Time Mode | 0:98 | 0-4 |
| Select Tuning Table | 25:6 | 0-16 |
| Arp Chance | 25:33 | conflicting, see below |
| Animate 1 Attack/Release | 25:34 / 25:35 | 0-127 |
| Animate 2 Attack/Release | 25:36 / 25:37 | 0-127 |
| LFO3 Phase/Slew/Fade | 25:38 / 25:39 / 25:40 | 0-120 / 0-127 / 0-127 |
| LFO4 Phase/Slew/Fade | 25:41 / 25:42 / 25:43 | 0-120 / 0-127 / 0-127 |
| Patch Cue, global | 64:0 | 0-1 |
| FM Osc3->1 manual | 25:13 | 0-127 |
| FM Osc1->2 manual | 25:17 | 0-127 |
| FM Osc2->3 manual | 25:21 | 0-127 |

`Atouch Scale` 64:3 is listed in the table but belongs to the addendum section explicitly marked Summit-only and is excluded from the Peak profile.

## High-confidence feature semantics

- Spread modes: Diverge, Alternate, Diverge 2, NoteVal.
- Pan display described as -64 to +63.
- Animate envelope timing is exponential; addendum examples are about 70 ms at 32, 600 ms at 64, 3.5 s at 96 and 15 s at 127.
- Peak envelope Delay is 0-127, with 127 approximately 10 seconds and around 85 approximately 1 second.
- Noise HPF is 0-127, initial 0.
- Chorus modes described in prose: Chorus, Flanger, Phlanger.
- Delay modes: Original, CrossFed, Dual.
- Delay output: PreDamp, PostDamp.
- Delay time modes: Normal, Double, Treble, QuadLoFi, HexVLoFi.
- Arp adds Chance and Chord 2 behavior.

These semantic statements still require raw-code confirmation where the numeric mapping is absent or conflicted.

## Conflicts that must remain visible in code

### Noise HPF

- MIDI table: 0-0, clearly unusable.
- 2.1 feature description: 0-127, initial 0.
- Policy: mark conflict; use 0-127 provisionally only after hardware send/receive verification.

### Arp Chance

- MIDI table: `01-100`.
- Addendum prose: 10-100.
- Current guide heading: 0-100; prose: 10-100.
- Policy: test raw 0, 1, 9, 10 and 100; record clamp/reject/display behavior.

### Pan

- User-facing range: -64 to +63.
- MIDI table: raw 0-127, default 0.
- Policy: no transform until tested.

### Chorus mode

- MIDI table label: Chorus/Phaser/Flanger.
- prose/current guide: Chorus/Flanger/Phlanger.
- Policy: map codes from actual display.

### Main modulation enums

- numeric table ranges are old.
- current label lists are larger.
- Policy: labels are valid; codes are unknown.

### Arp Type

- numeric table still 0-6.
- current modes include an added Chord 2.
- Policy: complete code capture required.

### KeySync and Animate Hold

- base PDF: 0-1.
- current web table: 0-127.
- Policy: semantic boolean, wire values captured.

### CC pairs

- official table gives paired controllers and 0-255-style ranges.
- no official byte algorithm found.
- Policy: dedicated capture and exhaustive boundary test.

## Third-party evidence boundary

KnobKraft Orm lists its Novation Summit/Peak adaptation as alpha and provides candidate SysEx constants. The project is AGPL by default with a commercial MIT option. It is not an official protocol source. Its values may be used to design clean-room tests but must not be copied or shipped without independent validation and license review.
