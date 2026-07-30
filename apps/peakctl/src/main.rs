use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use patchascent_midi_messages::{
    format_hex, ChannelMessage, DecodedMidiMessage, MidiChannel, RawMidiEvent,
};
use patchascent_midi_transport::{
    MidiBackend, MidirBackend, OutputScheduler, PacingProfile, SessionLogWriter, SessionMetadata,
    SessionRecord,
};
use patchascent_peak_domain::{Binding, ParameterId, ParameterRegistry};
use patchascent_peak_protocol::{
    encode_filter_resonance_test, CcPairAnalyzer, NrpnDiagnosticEvent, NrpnParser,
    NrpnParserConfig, S1LiveEditAcknowledgement, FILTER_RESONANCE_PARAMETER_ID,
};
#[cfg(feature = "nrpn_candidate_experimental")]
use patchascent_peak_protocol::{encode_oscillator_1_wave_candidate, NrpnEncodingStrategy};
use patchascent_peak_sync::{CommandClass, QueueContext, ScheduledCommand};
use patchascent_peak_sysex::{FramerEvent, SysExFramer, SysExFramerConfig};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "peakctl",
    version,
    about = "PatchAscent's read-first Novation Peak protocol laboratory",
    long_about = "Enumerate and monitor MIDI without guessing Peak protocol mappings. \
                  Live-edit tests are narrowly allowlisted and require explicit acknowledgement. \
                  No stored-memory, settings, firmware, or bootloader write command exists."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Enumerate input and output ports independently.
    Ports {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Monitor exact incoming bytes and decoded standard channel messages.
    Monitor {
        /// Stable input ID shown by `peakctl ports`.
        #[arg(long)]
        input: String,
        /// Directory for the timestamped JSONL session log.
        #[arg(long, default_value = "sessions")]
        log_dir: PathBuf,
        /// Stop after this many seconds; omit to run until Ctrl-C.
        #[arg(long)]
        duration_seconds: Option<u64>,
        /// Bounded callback queue capacity.
        #[arg(long, default_value_t = 8192)]
        queue_capacity: usize,
        #[command(flatten)]
        context: CaptureContext,
    },
    /// Capture F0..F7 messages opaquely, preserving exact bytes and hashes.
    CaptureSysex {
        /// Stable input ID shown by `peakctl ports`.
        #[arg(long)]
        input: String,
        /// Destination for the first message; later messages receive .partNN.
        #[arg(long)]
        output: PathBuf,
        /// Directory for the timestamped JSONL session log.
        #[arg(long, default_value = "sessions")]
        log_dir: PathBuf,
        /// Hard stop even if no `SysEx` arrives.
        #[arg(long, default_value_t = 120)]
        duration_seconds: u64,
        /// Stop after this many idle seconds once the first message completes.
        #[arg(long, default_value_t = 3)]
        idle_seconds: u64,
        /// Replace an existing destination file.
        #[arg(long)]
        overwrite: bool,
        #[command(flatten)]
        context: CaptureContext,
    },
    /// Receive-only analysis for an officially documented Peak CC pair.
    AnalyzeCcPair {
        /// Stable input ID shown by `peakctl ports`.
        #[arg(long)]
        input: String,
        /// First documented controller (Filter Frequency default: 29).
        #[arg(long, default_value_t = 29)]
        first_controller: u8,
        /// Second documented controller (Filter Frequency default: 61).
        #[arg(long, default_value_t = 61)]
        second_controller: u8,
        /// Stop after this many seconds; omit to run until Ctrl-C.
        #[arg(long)]
        duration_seconds: Option<u64>,
        /// Optional CSV table of observed members and prior counterpart values.
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Replace an existing CSV destination.
        #[arg(long, requires = "csv")]
        overwrite: bool,
        /// Directory for the timestamped JSONL session log.
        #[arg(long, default_value = "sessions")]
        log_dir: PathBuf,
        #[command(flatten)]
        context: CaptureContext,
    },
    /// Send the sole approved initial CC test: Filter Resonance, CC 79.
    SendFilterResonance {
        /// Stable output ID shown by `peakctl ports`.
        #[arg(long)]
        output: String,
        /// Peak MIDI channel in 1..=16.
        #[arg(long)]
        channel: u8,
        /// Raw CC value in 0..=127.
        #[arg(long)]
        value: u8,
        /// Confirm Settings > Protect is currently On.
        #[arg(long)]
        confirm_patch_protect_on: bool,
        /// Confirm this non-persistent command will change the current sound.
        #[arg(long = "i-understand-this-changes-the-current-sound")]
        acknowledge_live_edit: bool,
        /// Directory for the timestamped JSONL session log.
        #[arg(long, default_value = "sessions")]
        log_dir: PathBuf,
    },
    /// Experimental candidate for Oscillator 1 Wave NRPN 0:14.
    #[cfg(feature = "nrpn_candidate_experimental")]
    SendOscillator1Wave {
        /// Stable output ID shown by `peakctl ports`.
        #[arg(long)]
        output: String,
        /// Peak MIDI channel in 1..=16.
        #[arg(long)]
        channel: u8,
        /// Raw documented value in 0..=4.
        #[arg(long)]
        value: u8,
        /// Include CC38 Data Entry LSB; not yet Peak-verified.
        #[arg(long)]
        include_data_lsb: bool,
        /// Send 127/127 null-selector termination; not yet Peak-verified.
        #[arg(long)]
        terminate_with_null_selector: bool,
        /// Confirm Settings > Protect is currently On.
        #[arg(long)]
        confirm_patch_protect_on: bool,
        /// Confirm this non-persistent command will change the current sound.
        #[arg(long = "i-understand-this-changes-the-current-sound")]
        acknowledge_live_edit: bool,
        /// Directory for the timestamped JSONL session log.
        #[arg(long, default_value = "sessions")]
        log_dir: PathBuf,
    },
    /// Decode a complete MIDI message from hexadecimal bytes.
    Inspect {
        /// Example: `B0 4F 40`.
        #[arg(long)]
        hex: String,
    },
    /// Validate or summarize the evidence-governed parameter registry.
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum RegistryAction {
    Validate,
    Summary,
}

#[derive(Debug, Clone, Args)]
struct CaptureContext {
    /// Exact Settings > Version value. Omit only when still pending capture.
    #[arg(long)]
    peak_os_version: Option<String>,
    #[arg(long, value_enum)]
    connection: Option<ConnectionKind>,
    /// Peak MIDI channel in 1..=16.
    #[arg(long)]
    midi_channel: Option<u8>,
    #[arg(long, value_enum)]
    cc_nrpn_mode: Option<PeakMode>,
    #[arg(long, value_enum)]
    bank_patch_mode: Option<PeakMode>,
    #[arg(long, value_enum)]
    patch_protect: Option<PatchProtect>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConnectionKind {
    Usb,
    Din,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PeakMode {
    Disabled,
    Receive,
    Transmit,
    #[value(name = "rec+tran")]
    RecTran,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PatchProtect {
    On,
    Off,
}

impl ConnectionKind {
    const fn metadata_label(self) -> &'static str {
        match self {
            Self::Usb => "USB",
            Self::Din => "DIN",
        }
    }
}

impl PeakMode {
    const fn metadata_label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Receive => "Receive",
            Self::Transmit => "Transmit",
            Self::RecTran => "Rec+Tran",
        }
    }
}

impl PatchProtect {
    const fn metadata_label(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off",
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Ports { json } => list_ports(json),
        Command::Monitor {
            input,
            log_dir,
            duration_seconds,
            queue_capacity,
            context,
        } => monitor(&input, &log_dir, duration_seconds, queue_capacity, &context),
        Command::CaptureSysex {
            input,
            output,
            log_dir,
            duration_seconds,
            idle_seconds,
            overwrite,
            context,
        } => capture_sysex(CaptureSysexOptions {
            input: &input,
            output: &output,
            log_dir: &log_dir,
            duration: Duration::from_secs(duration_seconds),
            idle: Duration::from_secs(idle_seconds),
            overwrite,
            context: &context,
        }),
        Command::AnalyzeCcPair {
            input,
            first_controller,
            second_controller,
            duration_seconds,
            csv,
            overwrite,
            log_dir,
            context,
        } => analyze_cc_pair(AnalyzeCcPairOptions {
            input: &input,
            first_controller,
            second_controller,
            duration: duration_seconds.map(Duration::from_secs),
            csv: csv.as_deref(),
            overwrite,
            log_dir: &log_dir,
            context: &context,
        }),
        Command::SendFilterResonance {
            output,
            channel,
            value,
            confirm_patch_protect_on,
            acknowledge_live_edit,
            log_dir,
        } => send_filter_resonance(
            &output,
            channel,
            value,
            confirm_patch_protect_on,
            acknowledge_live_edit,
            &log_dir,
        ),
        #[cfg(feature = "nrpn_candidate_experimental")]
        Command::SendOscillator1Wave {
            output,
            channel,
            value,
            include_data_lsb,
            terminate_with_null_selector,
            confirm_patch_protect_on,
            acknowledge_live_edit,
            log_dir,
        } => send_oscillator_wave(
            &output,
            channel,
            value,
            NrpnEncodingStrategy {
                include_data_lsb,
                terminate_with_null_selector,
            },
            confirm_patch_protect_on,
            acknowledge_live_edit,
            &log_dir,
        ),
        Command::Inspect { hex } => inspect(&hex),
        Command::Registry { action } => registry(action),
    }
}

fn list_ports(json: bool) -> Result<()> {
    let inventory = MidirBackend.list_ports()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&inventory)?);
        return Ok(());
    }
    println!("INPUT PORTS");
    if inventory.inputs.is_empty() {
        println!("  (none)");
    }
    for port in inventory.inputs {
        println!("  {}  {}  [{}]", port.id, port.name, port.backend);
    }
    println!("OUTPUT PORTS");
    if inventory.outputs.is_empty() {
        println!("  (none)");
    }
    for port in inventory.outputs {
        println!("  {}  {}  [{}]", port.id, port.name, port.backend);
    }
    Ok(())
}

fn monitor(
    input_id: &str,
    log_dir: &Path,
    duration: Option<u64>,
    queue_capacity: usize,
    context: &CaptureContext,
) -> Result<()> {
    let session_id = Uuid::new_v4();
    let metadata = metadata(session_id, context);
    let mut log = SessionLogWriter::create(log_dir, &metadata)?;
    let session = MidirBackend.open_input(input_id, session_id, queue_capacity)?;
    let stop = install_stop_handler()?;
    let started = Instant::now();
    let duration = duration.map(Duration::from_secs);
    let mut raw_count = 0_u64;
    let mut nrpn = NrpnParser::new(NrpnParserConfig::default());
    let mut sysex = SysExFramer::new(SysExFramerConfig::default());

    println!(
        "Monitoring {} ({}) — press Ctrl-C to stop.",
        session.descriptor().name,
        session.descriptor().id
    );
    while !stop.load(Ordering::Relaxed) && duration.is_none_or(|limit| started.elapsed() < limit) {
        let Some(event) = session.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        raw_count += 1;
        print_raw_event(&event)?;
        log.append_raw(event.clone())?;
        log_derived_diagnostics(&mut log, &event, &mut nrpn, &mut sysex)?;
    }
    let dropped = session.dropped_count();
    session.close();
    finish_log(log, raw_count, dropped)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct CaptureSysexOptions<'a> {
    input: &'a str,
    output: &'a Path,
    log_dir: &'a Path,
    duration: Duration,
    idle: Duration,
    overwrite: bool,
    context: &'a CaptureContext,
}

fn capture_sysex(options: CaptureSysexOptions<'_>) -> Result<()> {
    ensure_destination_available(options.output, options.overwrite)?;
    let session_id = Uuid::new_v4();
    let metadata = metadata(session_id, options.context);
    let mut log = SessionLogWriter::create(options.log_dir, &metadata)?;
    let session = MidirBackend.open_input(options.input, session_id, 8192)?;
    let stop = install_stop_handler()?;
    let started = Instant::now();
    let mut last_capture: Option<Instant> = None;
    let mut framer = SysExFramer::new(SysExFramerConfig::default());
    let mut raw_count = 0_u64;
    let mut capture_count = 0_usize;

    println!(
        "Capturing opaque SysEx from {} — trigger Peak Backup > Go now.",
        session.descriptor().name
    );
    while !stop.load(Ordering::Relaxed)
        && started.elapsed() < options.duration
        && last_capture.is_none_or(|instant| instant.elapsed() < options.idle)
    {
        let Some(event) = session.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        raw_count += 1;
        log.append_raw(event.clone())?;
        for framed in framer.ingest(&event.bytes) {
            match framed {
                FramerEvent::Complete(message) => {
                    capture_count += 1;
                    let destination = numbered_destination(options.output, capture_count);
                    ensure_destination_available(&destination, options.overwrite)?;
                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    message.write_byte_identical(&destination)?;
                    println!(
                        "Captured {} bytes  sha256={}  {}",
                        message.bytes().len(),
                        message.sha256(),
                        destination.display()
                    );
                    log.append(&SessionRecord::SysexCaptured {
                        event_id: event.event_id,
                        byte_length: message.bytes().len(),
                        sha256: message.sha256().to_owned(),
                        identity: format!("{:?}", message.identity()),
                    })?;
                    last_capture = Some(Instant::now());
                }
                FramerEvent::Diagnostic(error) => {
                    log.append(&diagnostic("error", error.to_string()))?;
                    eprintln!("SysEx diagnostic: {error}");
                }
            }
        }
    }
    if let Some(error) = framer.finish() {
        log.append(&diagnostic("error", error.to_string()))?;
    }
    let dropped = session.dropped_count();
    session.close();
    finish_log(log, raw_count, dropped)?;
    if capture_count == 0 {
        bail!("no complete F0..F7 SysEx message was captured");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct AnalyzeCcPairOptions<'a> {
    input: &'a str,
    first_controller: u8,
    second_controller: u8,
    duration: Option<Duration>,
    csv: Option<&'a Path>,
    overwrite: bool,
    log_dir: &'a Path,
    context: &'a CaptureContext,
}

fn analyze_cc_pair(options: AnalyzeCcPairOptions<'_>) -> Result<()> {
    if options.first_controller > 127 || options.second_controller > 127 {
        bail!("CC controllers must be in 0..=127");
    }
    let session_id = Uuid::new_v4();
    let metadata = metadata(session_id, options.context);
    let mut log = SessionLogWriter::create(options.log_dir, &metadata)?;
    let session = MidirBackend.open_input(options.input, session_id, 8192)?;
    let stop = install_stop_handler()?;
    let started = Instant::now();
    let mut analyzer = CcPairAnalyzer::new(options.first_controller, options.second_controller);
    let mut raw_count = 0_u64;
    let mut csv_writer = match options.csv {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut open_options = OpenOptions::new();
            open_options.write(true);
            if options.overwrite {
                open_options.create(true).truncate(true);
            } else {
                open_options.create_new(true);
            }
            let file = open_options
                .open(path)
                .with_context(|| format!("failed to create CSV {}", path.display()))?;
            let mut writer = csv::Writer::from_writer(file);
            writer.write_record([
                "timestamp_micros",
                "channel",
                "member",
                "controller",
                "value",
                "prior_other_value",
                "prior_other_timestamp_micros",
            ])?;
            Some(writer)
        }
        None => None,
    };

    println!("timestamp_us\tch\tmember\tcc\tvalue\tprior_other");
    while !stop.load(Ordering::Relaxed)
        && options
            .duration
            .is_none_or(|limit| started.elapsed() < limit)
    {
        let Some(event) = session.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        raw_count += 1;
        log.append_raw(event.clone())?;
        let DecodedMidiMessage::Channel(message) = event.decoded() else {
            continue;
        };
        let Some(observation) = analyzer.ingest(&message, event.monotonic_timestamp_micros) else {
            continue;
        };
        println!(
            "{}\t{}\t{:?}\t{}\t{}\t{:?}",
            observation.timestamp_micros,
            observation.channel.one_based(),
            observation.member,
            observation.controller,
            observation.value,
            observation.prior_other_member
        );
        if let Some(writer) = csv_writer.as_mut() {
            let (prior_value, prior_timestamp) = observation
                .prior_other_member
                .map_or((String::new(), String::new()), |(value, timestamp)| {
                    (value.to_string(), timestamp.to_string())
                });
            writer.write_record([
                observation.timestamp_micros.to_string(),
                observation.channel.one_based().to_string(),
                format!("{:?}", observation.member),
                observation.controller.to_string(),
                observation.value.to_string(),
                prior_value,
                prior_timestamp,
            ])?;
            writer.flush()?;
        }
    }
    let dropped = session.dropped_count();
    session.close();
    finish_log(log, raw_count, dropped)?;
    Ok(())
}

fn send_filter_resonance(
    output_id: &str,
    channel: u8,
    value: u8,
    patch_protect_confirmed: bool,
    acknowledged: bool,
    log_dir: &Path,
) -> Result<()> {
    require_patch_protect(patch_protect_confirmed)?;
    let acknowledgement = S1LiveEditAcknowledgement::from_cli_flag(acknowledged)?;
    let channel = MidiChannel::from_one_based(channel)?;
    let message = encode_filter_resonance_test(channel, value, acknowledgement)?;
    send_allowlisted_sequence(
        output_id,
        channel,
        ParameterId::new(FILTER_RESONANCE_PARAMETER_ID)?,
        vec![message.to_vec()],
        log_dir,
    )
}

#[cfg(feature = "nrpn_candidate_experimental")]
fn send_oscillator_wave(
    output_id: &str,
    channel: u8,
    value: u8,
    strategy: NrpnEncodingStrategy,
    patch_protect_confirmed: bool,
    acknowledged: bool,
    log_dir: &Path,
) -> Result<()> {
    require_patch_protect(patch_protect_confirmed)?;
    let acknowledgement = S1LiveEditAcknowledgement::from_cli_flag(acknowledged)?;
    let channel = MidiChannel::from_one_based(channel)?;
    let messages = encode_oscillator_1_wave_candidate(channel, value, strategy, acknowledgement)?
        .into_iter()
        .map(|message| message.to_vec())
        .collect();
    send_allowlisted_sequence(
        output_id,
        channel,
        ParameterId::new("oscillators.oscillator_1_wave")?,
        messages,
        log_dir,
    )
}

fn send_allowlisted_sequence(
    output_id: &str,
    channel: MidiChannel,
    parameter_id: ParameterId,
    messages: Vec<Vec<u8>>,
    log_dir: &Path,
) -> Result<()> {
    let session_id = Uuid::new_v4();
    let context = CaptureContext {
        peak_os_version: None,
        connection: None,
        midi_channel: Some(channel.one_based()),
        cc_nrpn_mode: Some(PeakMode::Receive),
        bank_patch_mode: None,
        patch_protect: Some(PatchProtect::On),
    };
    let metadata = metadata(session_id, &context);
    let mut log = SessionLogWriter::create(log_dir, &metadata)?;
    let output = MidirBackend.open_output(output_id, session_id)?;
    let command_class = if messages.len() == 1 {
        CommandClass::AtomicSingle
    } else {
        CommandClass::AtomicSequence
    };
    let scheduler = OutputScheduler::start(output, 128, PacingProfile::default());
    let command = ScheduledCommand {
        sequence_id: 1,
        context: QueueContext {
            session_id,
            patch_epoch: 0,
        },
        class: command_class,
        parameter_id: Some(parameter_id),
        messages,
        enqueued_at_micros: 0,
    };
    scheduler.submit(command)?;
    let receipt = scheduler.wait(1, Duration::from_secs(5))?;
    for event in receipt.raw_events {
        print_raw_event(&event)?;
        log.append_raw(event)?;
    }
    let metrics = scheduler.status().metrics;
    scheduler.shutdown();
    log.append(&SessionRecord::SessionFinished {
        timestamp: Utc::now(),
        raw_event_count: metrics.sent_message_count,
        dropped_event_count: 0,
    })?;
    let summary = log.finalize()?;
    println!(
        "Session log: {}  sha256={}",
        summary.path.display(),
        summary.sha256
    );
    Ok(())
}

fn inspect(hex: &str) -> Result<()> {
    let bytes = parse_hex_bytes(hex)?;
    let decoded = patchascent_midi_messages::decode_message(&bytes);
    println!("hex:     {}", format_hex(&bytes));
    println!(
        "decimal: {}",
        bytes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("decoded: {}", serde_json::to_string_pretty(&decoded)?);
    Ok(())
}

fn registry(action: RegistryAction) -> Result<()> {
    let registry = ParameterRegistry::embedded()?;
    registry.validate_seed_safety()?;
    let mapped = registry.binding_index().len();
    match action {
        RegistryAction::Validate => {
            println!(
                "OK: {} parameters; {} unique mapped bindings; all live writes disabled.",
                registry.parameters.len(),
                mapped
            );
        }
        RegistryAction::Summary => {
            let mut cc = 0;
            let mut cc_pair = 0;
            let mut nrpn = 0;
            let mut unmapped = 0;
            let mut conflicts = 0;
            for parameter in &registry.parameters {
                match parameter.binding {
                    Binding::Cc { .. } => cc += 1,
                    Binding::CcPair { .. } => cc_pair += 1,
                    Binding::Nrpn { .. } => nrpn += 1,
                    Binding::Unmapped | Binding::Unknown => unmapped += 1,
                }
                if parameter.evidence.status.contains("conflict")
                    || parameter.evidence.status.contains("stale")
                {
                    conflicts += 1;
                }
            }
            println!("parameters: {}", registry.parameters.len());
            println!("cc: {cc}");
            println!("cc_pair_disabled: {cc_pair}");
            println!("nrpn_documented: {nrpn}");
            println!("unmapped: {unmapped}");
            println!("conflicted_or_stale: {conflicts}");
            println!("live_writes_enabled: 0");
        }
    }
    Ok(())
}

fn log_derived_diagnostics(
    log: &mut SessionLogWriter,
    event: &RawMidiEvent,
    nrpn: &mut NrpnParser,
    sysex: &mut SysExFramer,
) -> Result<()> {
    if let DecodedMidiMessage::Channel(message) = event.decoded() {
        for diagnostic_event in nrpn.ingest(&message, event.monotonic_timestamp_micros) {
            log.append(&diagnostic(
                "info",
                serde_json::to_string(&diagnostic_event)?,
            ))?;
            if let NrpnDiagnosticEvent::DataEntry { .. } = diagnostic_event {
                println!("  NRPN {}", serde_json::to_string(&diagnostic_event)?);
            }
        }
    }
    for framed in sysex.ingest(&event.bytes) {
        match framed {
            FramerEvent::Complete(message) => {
                println!(
                    "  SysEx complete: {} bytes sha256={}",
                    message.bytes().len(),
                    message.sha256()
                );
                log.append(&SessionRecord::SysexCaptured {
                    event_id: event.event_id,
                    byte_length: message.bytes().len(),
                    sha256: message.sha256().to_owned(),
                    identity: format!("{:?}", message.identity()),
                })?;
            }
            FramerEvent::Diagnostic(error) => {
                log.append(&diagnostic("warning", error.to_string()))?;
            }
        }
    }
    Ok(())
}

fn print_raw_event(event: &RawMidiEvent) -> Result<()> {
    let decoded = event.decoded();
    let decimals = event
        .bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "{:>12}us  {:<23}  [{:<16}]  {}",
        event.monotonic_timestamp_micros,
        format_hex(&event.bytes),
        decimals,
        compact_interpretation(&decoded)?
    );
    Ok(())
}

fn compact_interpretation(decoded: &DecodedMidiMessage) -> Result<String> {
    if let DecodedMidiMessage::Channel(ChannelMessage::ControlChange {
        channel,
        controller,
        value,
    }) = decoded
    {
        return Ok(format!(
            "CC ch={} controller={} value={}",
            channel.one_based(),
            controller,
            value
        ));
    }
    Ok(serde_json::to_string(decoded)?)
}

fn metadata(session_id: Uuid, context: &CaptureContext) -> SessionMetadata {
    SessionMetadata {
        session_id,
        started_at: Utc::now(),
        application: "peakctl".to_owned(),
        application_version: env!("CARGO_PKG_VERSION").to_owned(),
        peak_os_version: context.peak_os_version.clone(),
        computer_os: std::env::consts::OS.to_owned(),
        connection: context.connection.map_or_else(
            || "pending capture".to_owned(),
            |value| value.metadata_label().to_owned(),
        ),
        midi_channel: context.midi_channel,
        cc_nrpn_mode: context
            .cc_nrpn_mode
            .map(|value| value.metadata_label().to_owned()),
        bank_patch_mode: context
            .bank_patch_mode
            .map(|value| value.metadata_label().to_owned()),
        patch_protect: context
            .patch_protect
            .map(|value| value.metadata_label().to_owned()),
    }
}

fn diagnostic(severity: &str, message: String) -> SessionRecord {
    SessionRecord::Diagnostic {
        timestamp: Utc::now(),
        severity: severity.to_owned(),
        message,
    }
}

fn finish_log(mut log: SessionLogWriter, raw_count: u64, dropped_count: u64) -> Result<()> {
    log.append(&SessionRecord::SessionFinished {
        timestamp: Utc::now(),
        raw_event_count: raw_count,
        dropped_event_count: dropped_count,
    })?;
    let summary = log.finalize()?;
    println!(
        "Session log: {}  sha256={}  records={}  dropped={}",
        summary.path.display(),
        summary.sha256,
        summary.records,
        dropped_count
    );
    Ok(())
}

fn install_stop_handler() -> Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))
        .context("failed to install Ctrl-C handler")?;
    Ok(stop)
}

fn parse_hex_bytes(value: &str) -> Result<Vec<u8>> {
    value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let part = part
                .strip_prefix("0x")
                .or_else(|| part.strip_prefix("0X"))
                .unwrap_or(part);
            u8::from_str_radix(part, 16)
                .with_context(|| format!("invalid hexadecimal byte {part:?}"))
        })
        .collect()
}

fn ensure_destination_available(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        bail!(
            "{} already exists; pass --overwrite to replace it",
            path.display()
        );
    }
    Ok(())
}

fn numbered_destination(base: &Path, index: usize) -> PathBuf {
    if index == 1 {
        return base.to_owned();
    }
    let parent = base.parent().unwrap_or_else(|| Path::new(""));
    let stem = base
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("capture");
    let extension = base
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("syx");
    parent.join(format!("{stem}.part{index:02}.{extension}"))
}

fn require_patch_protect(confirmed: bool) -> Result<()> {
    if !confirmed {
        bail!(
            "refusing live test: confirm Settings > Protect is On with \
             --confirm-patch-protect-on"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_hex_notation() {
        assert_eq!(
            parse_hex_bytes("0xB0, 4F 40").unwrap(),
            vec![0xB0, 0x4F, 0x40]
        );
    }

    #[test]
    fn multiple_sysex_destinations_are_deterministic() {
        let base = Path::new("current.syx");
        assert_eq!(numbered_destination(base, 1), PathBuf::from("current.syx"));
        assert_eq!(
            numbered_destination(base, 2),
            PathBuf::from("current.part02.syx")
        );
    }

    #[test]
    fn command_surface_has_no_persistent_write_operation() {
        let command = <Cli as clap::CommandFactory>::command();
        let subcommands = command
            .get_subcommands()
            .map(clap::Command::get_name)
            .collect::<Vec<_>>();
        assert!(!subcommands.iter().any(|name| {
            name.contains("memory")
                || name.contains("firmware")
                || name.contains("bootloader")
                || name.contains("store")
                || name.contains("write")
        }));
    }

    #[cfg(not(feature = "nrpn_candidate_experimental"))]
    #[test]
    fn default_build_excludes_candidate_nrpn_sender() {
        let command = <Cli as clap::CommandFactory>::command();
        assert!(!command
            .get_subcommands()
            .any(|subcommand| subcommand.get_name() == "send-oscillator1-wave"));
    }
}
