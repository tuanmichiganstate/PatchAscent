# Hardware capture and upload guide

The owner offered to collect files manually. These captures unlock the uncertain parts of the editor without requiring any destructive write.

## Safety preparation

1. Back up important Peak patches in Novation Components or through the Peak Backup menu.
2. Set `Settings > Protect` to On.
3. Connect Peak directly by USB for the first baseline, avoiding a hub where practical.
4. Note the MIDI channel.
5. Set `CC/NRPN` to `Rec+Tran` for bidirectional tests, or `Transmit` for receive-only capture.
6. Set `Bank/Patch` to `Rec+Tran` or `Transmit` for patch-selection capture.
7. Do not enter bootloader mode and do not run calibration.

## Capture set A: device identity

Provide:

- Photo or screenshot of `Settings > Version` with the exact OS value legible.
- Screenshot or text list of MIDI input/output port names on the computer.
- Computer OS version and connection type.

Suggested names:

```text
A01_peak_version.jpg
A02_midi_ports.txt
A03_environment.txt
```

## Capture set B: raw CC/NRPN traffic

Using the project's protocol laboratory or a MIDI monitor that can save raw bytes, record each control separately. Start and stop a new log for each test.

1. Filter Resonance: minimum, slow sweep, maximum. Expected official binding: CC79.
2. Filter Frequency: minimum, several small adjacent steps around low/middle/high values, maximum. Official table calls it CC pair 29,61.
3. Oscillator 1 Wave: select every waveform. Expected NRPN 0:14.
4. Pan Position: values displayed at far left, -1, 0, +1, far right. Expected NRPN 0:51.
5. Arp Chance: attempt/display 0, 1, 9, 10 and 100 where the hardware allows it. Expected NRPN 25:33.
6. Chorus mode: cycle all three displayed names. Expected NRPN 0:115.
7. KeySync and Animate Hold: toggle off/on to reveal whether Peak sends 1 or 127.

Do not move unrelated controls during a capture.

## Capture set C: Current patch SysEx

On Peak:

1. Load a patch that may safely be inspected.
2. Open Settings > Backup.
3. Set `Select` to `Current`.
4. Set `Send To` to `USBport`.
5. Start SysEx capture on the computer.
6. Select `Go` on Peak.
7. Stop and save capture.
8. Repeat without changing anything.

Suggested names:

```text
C01_current_repeat1.syx
C02_current_repeat2.syx
```

The two files reveal whether unchanged dumps contain dynamic bytes.

## Capture set D: stored patch, bank and settings

- Capture one known stored patch if the available software supports it, or provide a Bank A backup from the Peak Backup menu.
- Capture `Settings` separately.

Suggested names:

```text
D01_bank_A.syx
D02_settings.syx
```

A full bank is useful but not required to begin the edit-buffer phase.

## Capture set E: differential patch dumps

Only after a baseline Current dump is captured:

1. Start from the same baseline patch each time.
2. Change exactly one parameter.
3. Dump Current.
4. Restore baseline before the next parameter.

High-value first parameters:

- patch name only
- Filter Resonance 0 then 1
- Oscillator 1 Wave two adjacent values
- Voice Pan adjacent display values
- one Mod Matrix slot Source A
- the same slot Destination
- the same slot Depth
- Envelope Delay 0 then 1

Suggested naming:

```text
E_filter_res_raw000.syx
E_filter_res_raw001.syx
E_mod1_source_Direct.syx
E_mod1_source_ModWheel.syx
```

## Manifest

Include a `capture_manifest.json` validated against `schemas/capture_manifest.schema.json`. Record:

- exact Peak OS version
- date/time
- computer OS
- USB or DIN
- MIDI channel
- CC/NRPN and Bank/Patch modes
- Patch Protect state
- file SHA-256 values
- any unexpected display or sound behavior

## What not to do yet

- Do not send third-party SysEx write commands.
- Do not disable Patch Protect for early captures.
- Do not modify a stored patch merely to test writing.
- Do not assume a captured bank file can safely be edited and returned.
- Do not upload personal project folders; only the named captures and manifest are needed.
