import { useEffect, useMemo, useState } from "react";
import { PARAMETER_REGISTRY } from "@patchascent/parameter-registry";

import { BrandMark } from "./components/BrandMark";
import { Metric } from "./components/Metric";
import { StatusPill } from "./components/StatusPill";
import {
  getLaboratoryStatus,
  getMidiPorts,
  getMonitorStatus,
  isDesktopRuntime,
  listenForMonitorState,
  listenForRawMidi,
  startMidiMonitor,
  stopMidiMonitor,
} from "./lib/tauri";
import type {
  LaboratoryStatus,
  MonitorEvent,
  MonitorStatus,
  PortInventory,
} from "./types";

const EMPTY_PORTS: PortInventory = { inputs: [], outputs: [] };
const EMPTY_MONITOR: MonitorStatus = {
  active: false,
  inputPortId: null,
  inputPortName: null,
  sessionId: null,
  error: null,
};
const MAX_EVENTS = 300;

export function App() {
  const [laboratory, setLaboratory] = useState<LaboratoryStatus | null>(null);
  const [ports, setPorts] = useState<PortInventory>(EMPTY_PORTS);
  const [selectedInput, setSelectedInput] = useState("");
  const [monitor, setMonitor] = useState<MonitorStatus>(EMPTY_MONITOR);
  const [events, setEvents] = useState<readonly MonitorEvent[]>([]);
  const [filter, setFilter] = useState("");
  const [showSystem, setShowSystem] = useState(true);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [checklist, setChecklist] = useState({
    version: false,
    protect: false,
    ccNrpn: false,
    bankPatch: false,
  });

  const desktopRuntime = isDesktopRuntime();

  useEffect(() => {
    void Promise.all([getLaboratoryStatus(), getMidiPorts(), getMonitorStatus()])
      .then(([status, inventory, monitorStatus]) => {
        setLaboratory(status);
        setPorts(inventory);
        setMonitor(monitorStatus);
        setSelectedInput((current) => current || inventory.inputs[0]?.id || "");
      })
      .catch((error: unknown) => {
        setNotice(toMessage(error));
      });
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenRaw: (() => void) | undefined;
    let unlistenState: (() => void) | undefined;

    void listenForRawMidi((event) => {
      if (!disposed) {
        setEvents((current) => [event, ...current].slice(0, MAX_EVENTS));
      }
    }).then((unlisten) => {
      unlistenRaw = unlisten;
    });
    void listenForMonitorState((status) => {
      if (!disposed) {
        setMonitor(status);
      }
    }).then((unlisten) => {
      unlistenState = unlisten;
    });

    return () => {
      disposed = true;
      unlistenRaw?.();
      unlistenState?.();
    };
  }, []);

  const visibleEvents = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return events.filter((event) => {
      if (!showSystem && event.decoded.kind === "system") {
        return false;
      }
      if (!needle) {
        return true;
      }
      const haystack = [
        event.hex,
        event.event.port_name,
        event.event.direction,
        summarizeDecoded(event.decoded),
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(needle);
    });
  }, [events, filter, showSystem]);

  const evidenceCounts = useMemo(() => {
    const conflicted = PARAMETER_REGISTRY.filter(
      (parameter) =>
        parameter.evidence.status.includes("conflict") ||
        parameter.evidence.status.includes("stale"),
    ).length;
    const excluded = PARAMETER_REGISTRY.filter(
      (parameter) => parameter.gates.implementation === "exclude_from_peak_build",
    ).length;
    return { conflicted, excluded };
  }, []);

  async function refreshPorts() {
    setBusy(true);
    setNotice(null);
    try {
      const inventory = await getMidiPorts();
      setPorts(inventory);
      setSelectedInput((current) => {
        const stillPresent = inventory.inputs.some((port) => port.id === current);
        return stillPresent ? current : inventory.inputs[0]?.id || "";
      });
    } catch (error) {
      setNotice(toMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function toggleMonitor() {
    setBusy(true);
    setNotice(null);
    try {
      if (monitor.active) {
        setMonitor(await stopMidiMonitor());
      } else {
        if (!selectedInput) {
          throw new Error("Select an input port before starting the monitor.");
        }
        setEvents([]);
        setMonitor(await startMidiMonitor(selectedInput));
      }
    } catch (error) {
      setNotice(toMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="shell">
      <header className="topbar">
        <a className="brand" href="#main" aria-label="PatchAscent home">
          <BrandMark />
          <span>
            <strong>PatchAscent</strong>
            <small>Peak protocol laboratory</small>
          </span>
        </a>
        <div className="topbar__center">
          <StatusPill tone="neutral">M0 · read first</StatusPill>
          <span className="topbar__divider" />
          <span className="topbar__copy">One Peak · USB baseline · Firmware 2.1 feature set</span>
        </div>
        <div className="runtime-badge">
          <span className={`runtime-badge__dot ${desktopRuntime ? "is-live" : ""}`} />
          {desktopRuntime ? "Desktop runtime" : "Browser preview"}
        </div>
      </header>

      <div className="safety-rail">
        <span className="safety-rail__label">Safety boundary</span>
        <span><i className="rail-check">✓</i> Passive input enabled</span>
        <span><i className="rail-lock">×</i> CC-pair writes disabled</span>
        <span><i className="rail-lock">×</i> SysEx writes absent</span>
        <span><i className="rail-lock">×</i> Memory writes absent</span>
      </div>

      <main id="main" className="workspace">
        <section className="hero-panel">
          <div>
            <p className="eyebrow">Evidence before control</p>
            <h1>Listen to the instrument.<br />Keep every byte.</h1>
            <p className="hero-panel__lede">
              PatchAscent begins as a protocol laboratory. This shell can identify ports and
              monitor raw traffic; the complete synth editor remains gated until hardware
              tests HV-001 through HV-014 pass.
            </p>
          </div>
          <div className="hero-panel__gate">
            <span className="gate-ring">
              <strong>0</strong>
              <small>/ 14</small>
            </span>
            <div>
              <StatusPill tone="pending">Hardware evidence pending</StatusPill>
              <p>{laboratory?.hardwareGate ?? "Loading hardware gate…"}</p>
            </div>
          </div>
        </section>

        {notice ? (
          <div className="notice" role="alert">
            <span>!</span>
            <p>{notice}</p>
            <button type="button" onClick={() => setNotice(null)} aria-label="Dismiss notice">
              Dismiss
            </button>
          </div>
        ) : null}

        <div className="grid grid--primary">
          <section className="panel ports-panel">
            <div className="panel__heading">
              <div>
                <p className="eyebrow">01 · transport</p>
                <h2>MIDI ports</h2>
              </div>
              <button className="button button--quiet" type="button" onClick={() => void refreshPorts()} disabled={busy}>
                Refresh
              </button>
            </div>

            <label className="field-label" htmlFor="input-port">Input port</label>
            <div className="select-wrap">
              <select
                id="input-port"
                value={selectedInput}
                onChange={(event) => setSelectedInput(event.target.value)}
                disabled={monitor.active || busy}
              >
                <option value="">Select input…</option>
                {ports.inputs.map((port) => (
                  <option key={port.id} value={port.id}>
                    {port.name} · {port.backend}
                  </option>
                ))}
              </select>
            </div>
            <PortDetail
              label="Input"
              port={ports.inputs.find((port) => port.id === selectedInput)}
            />

            <div className="port-divider" />

            <div className="output-summary">
              <div>
                <span>Output ports discovered</span>
                <strong>{ports.outputs.length}</strong>
              </div>
              <p>
                Output is intentionally not exposed in this diagnostics shell. Approved live
                tests remain isolated in <code>peakctl</code>.
              </p>
            </div>

            <button
              className={`button button--monitor ${monitor.active ? "is-active" : ""}`}
              type="button"
              onClick={() => void toggleMonitor()}
              disabled={busy || (!selectedInput && !monitor.active) || !desktopRuntime}
            >
              <span className="button__indicator" />
              {monitor.active ? "Stop raw monitor" : "Start raw monitor"}
            </button>
            {!desktopRuntime ? (
              <p className="field-note">Open the Tauri app to enumerate physical MIDI ports.</p>
            ) : null}
          </section>

          <section className="panel checklist-panel">
            <div className="panel__heading">
              <div>
                <p className="eyebrow">02 · readiness</p>
                <h2>Peak checklist</h2>
              </div>
              <StatusPill tone={Object.values(checklist).every(Boolean) ? "ready" : "pending"}>
                {Object.values(checklist).filter(Boolean).length} / 4
              </StatusPill>
            </div>
            <p className="panel__intro">
              These are operator confirmations, not inferred device state. They are not saved
              as protocol evidence.
            </p>
            <ChecklistRow
              checked={checklist.version}
              title="Exact OS build recorded"
              detail="Settings › Version photo or exact text"
              onChange={(value) => setChecklist((current) => ({ ...current, version: value }))}
            />
            <ChecklistRow
              checked={checklist.protect}
              title="Patch Protect is On"
              detail="Required throughout non-destructive research"
              onChange={(value) => setChecklist((current) => ({ ...current, protect: value }))}
            />
            <ChecklistRow
              checked={checklist.ccNrpn}
              title="CC / NRPN mode confirmed"
              detail="Transmit for passive capture; Rec+Tran for bidirectional tests"
              onChange={(value) => setChecklist((current) => ({ ...current, ccNrpn: value }))}
            />
            <ChecklistRow
              checked={checklist.bankPatch}
              title="Bank / Patch mode confirmed"
              detail="Separate from parameter data and patch synchronization"
              onChange={(value) => setChecklist((current) => ({ ...current, bankPatch: value }))}
            />
          </section>

          <section className="panel evidence-panel">
            <div className="panel__heading">
              <div>
                <p className="eyebrow">03 · registry</p>
                <h2>Evidence inventory</h2>
              </div>
              <StatusPill tone="blocked">Writes 0</StatusPill>
            </div>
            <div className="metric-grid">
              <Metric value={laboratory?.parameterCount ?? "—"} label="Definitions" detail="Canonical seed" />
              <Metric value={laboratory?.ccCount ?? "—"} label="CC mappings" detail="Documented" />
              <Metric value={laboratory?.documentedNrpnCount ?? "—"} label="NRPN mappings" detail="Candidate family" />
              <Metric value={laboratory?.disabledCcPairCount ?? "—"} label="CC pairs" detail="All quarantined" />
            </div>
            <div className="evidence-lines">
              <div><span>Conflicted or stale records</span><strong>{evidenceCounts.conflicted}</strong></div>
              <div><span>Excluded Summit-only records</span><strong>{evidenceCounts.excluded}</strong></div>
              <div><span>Mapped bindings</span><strong>{laboratory?.mappedBindingCount ?? "—"}</strong></div>
              <div><span>Stored-memory write API</span><strong className="safe-absence">Not present</strong></div>
            </div>
          </section>
        </div>

        <section className="panel monitor-panel">
          <div className="panel__heading monitor-heading">
            <div>
              <p className="eyebrow">04 · raw event stream</p>
              <h2>MIDI monitor</h2>
            </div>
            <div className="monitor-state">
              <span className={`monitor-state__pulse ${monitor.active ? "is-active" : ""}`} />
              <div>
                <strong>{monitor.active ? "Listening" : "Idle"}</strong>
                <small>{monitor.inputPortName ?? "No input session"}</small>
              </div>
            </div>
          </div>

          <div className="monitor-toolbar">
            <label className="search-field">
              <span>Filter</span>
              <input
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
                placeholder="CC, port, hex, message type…"
              />
            </label>
            <label className="toggle-field">
              <input
                type="checkbox"
                checked={showSystem}
                onChange={(event) => setShowSystem(event.target.checked)}
              />
              <span>System messages</span>
            </label>
            <span className="event-count">{visibleEvents.length} / {events.length} visible</span>
            <button className="button button--quiet" type="button" onClick={() => setEvents([])} disabled={events.length === 0}>
              Clear
            </button>
          </div>

          <div className="event-table" role="region" aria-label="Raw MIDI events" tabIndex={0}>
            <div className="event-row event-row--header">
              <span>Time</span>
              <span>Direction / port</span>
              <span>Raw bytes</span>
              <span>Interpretation</span>
            </div>
            {visibleEvents.length === 0 ? (
              <div className="event-empty">
                <div className="event-empty__glyph">
                  <span />
                  <span />
                  <span />
                </div>
                <strong>{monitor.active ? "Waiting for Peak traffic" : "No raw session is active"}</strong>
                <p>Every callback will retain timestamp, port, channel, and exact bytes.</p>
              </div>
            ) : (
              visibleEvents.map((event) => (
                <div className="event-row" key={`${event.event.session_id}-${event.event.event_id}`}>
                  <span className="mono">{formatMicros(event.event.monotonic_timestamp_micros)}</span>
                  <span>
                    <b className={`direction direction--${event.event.direction}`}>
                      {event.event.direction}
                    </b>
                    <small>{event.event.port_name}</small>
                  </span>
                  <span className="mono raw-hex">{event.hex}</span>
                  <span className="event-meaning">{summarizeDecoded(event.decoded)}</span>
                </div>
              ))
            )}
          </div>
        </section>

        <section className="next-gate">
          <div>
            <p className="eyebrow">Next evidence unlock</p>
            <h2>Filter Resonance · CC 79</h2>
            <p>
              Capture the physical control first (HV-005), then run the six allowlisted values
              through <code>peakctl</code> for HV-006. UI control remains disabled until both
              directions and semantics are recorded.
            </p>
          </div>
          <div className="gate-sequence" aria-label="Verification sequence">
            <span className="gate-step is-current"><i>1</i>Receive</span>
            <span className="gate-line" />
            <span className="gate-step"><i>2</i>Send</span>
            <span className="gate-line" />
            <span className="gate-step"><i>3</i>Semantic</span>
            <span className="gate-line" />
            <span className="gate-step"><i>4</i>Round trip</span>
          </div>
        </section>
      </main>

      <footer>
        <span>PatchAscent 0.1.0 · Milestone 0</span>
        <span>Unknown means unknown. No documentary defaults are transmitted.</span>
      </footer>
    </div>
  );
}

interface ChecklistRowProps {
  readonly checked: boolean;
  readonly title: string;
  readonly detail: string;
  readonly onChange: (value: boolean) => void;
}

function ChecklistRow({ checked, title, detail, onChange }: ChecklistRowProps) {
  return (
    <label className={`check-row ${checked ? "is-checked" : ""}`}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="check-row__box">{checked ? "✓" : ""}</span>
      <span>
        <strong>{title}</strong>
        <small>{detail}</small>
      </span>
    </label>
  );
}

function PortDetail({
  label,
  port,
}: {
  readonly label: string;
  readonly port:
    | {
        readonly id: string;
        readonly name: string;
        readonly backend: string;
      }
    | undefined;
}) {
  return (
    <div className="port-detail">
      <span className="port-detail__jack" aria-hidden="true"><i /><i /><i /></span>
      <div>
        <strong>{port?.name ?? `${label} not selected`}</strong>
        <code>{port?.id ?? "—"}</code>
      </div>
      <StatusPill tone={port ? "neutral" : "pending"}>{port?.backend ?? "Pending"}</StatusPill>
    </div>
  );
}

function summarizeDecoded(decoded: MonitorEvent["decoded"]): string {
  if (decoded.kind === "unknown") {
    return `Unknown · ${decoded.message.reason}`;
  }
  const type = typeof decoded.message.type === "string" ? decoded.message.type : decoded.kind;
  const channel =
    typeof decoded.message.channel === "number"
      ? ` · ch ${decoded.message.channel}`
      : "";
  const controller =
    typeof decoded.message.controller === "number"
      ? ` · CC ${decoded.message.controller}`
      : "";
  const value =
    typeof decoded.message.value === "number"
      ? ` = ${decoded.message.value}`
      : "";
  return `${humanize(type)}${channel}${controller}${value}`;
}

function humanize(value: string): string {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatMicros(value: number): string {
  return `${(value / 1_000_000).toFixed(3)}s`;
}

function toMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
