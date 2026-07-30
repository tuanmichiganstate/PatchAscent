import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PARAMETER_REGISTRY } from "@patchascent/parameter-registry";

import type {
  LaboratoryStatus,
  MonitorEvent,
  MonitorStatus,
  PortInventory,
} from "../types";

export const RAW_MIDI_EVENT = "patchascent://raw-midi";
export const MONITOR_STATE_EVENT = "patchascent://monitor-state";

export function isDesktopRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function getLaboratoryStatus(): Promise<LaboratoryStatus> {
  if (isDesktopRuntime()) {
    return invoke<LaboratoryStatus>("laboratory_status");
  }
  const bindings = PARAMETER_REGISTRY.filter(
    (parameter) => parameter.binding.kind !== "unmapped",
  );
  return {
    milestone: "M0 · Protocol Laboratory",
    parameterCount: PARAMETER_REGISTRY.length,
    mappedBindingCount: bindings.length,
    ccCount: PARAMETER_REGISTRY.filter((parameter) => parameter.binding.kind === "cc").length,
    disabledCcPairCount: PARAMETER_REGISTRY.filter(
      (parameter) => parameter.binding.kind === "cc_pair",
    ).length,
    documentedNrpnCount: PARAMETER_REGISTRY.filter(
      (parameter) => parameter.binding.kind === "nrpn",
    ).length,
    enabledLiveWriteCount: PARAMETER_REGISTRY.filter(
      (parameter) => parameter.gates.live_write_enabled,
    ).length,
    hardwareWriteApiPresent: false,
    exactPeakOsBuild: null,
    hardwareGate: "HV-001 through HV-014 pending physical Peak",
  };
}

export async function getMidiPorts(): Promise<PortInventory> {
  if (!isDesktopRuntime()) {
    return { inputs: [], outputs: [] };
  }
  return invoke<PortInventory>("list_midi_ports");
}

export async function startMidiMonitor(inputPortId: string): Promise<MonitorStatus> {
  if (!isDesktopRuntime()) {
    throw new Error("MIDI monitoring is available in the PatchAscent desktop runtime.");
  }
  return invoke<MonitorStatus>("start_monitor", { inputPortId });
}

export async function stopMidiMonitor(): Promise<MonitorStatus> {
  if (!isDesktopRuntime()) {
    return emptyMonitorStatus();
  }
  return invoke<MonitorStatus>("stop_monitor");
}

export async function getMonitorStatus(): Promise<MonitorStatus> {
  if (!isDesktopRuntime()) {
    return emptyMonitorStatus();
  }
  return invoke<MonitorStatus>("monitor_status");
}

export async function listenForRawMidi(
  onEvent: (event: MonitorEvent) => void,
): Promise<UnlistenFn> {
  if (!isDesktopRuntime()) {
    return () => undefined;
  }
  return listen<MonitorEvent>(RAW_MIDI_EVENT, ({ payload }) => onEvent(payload));
}

export async function listenForMonitorState(
  onEvent: (event: MonitorStatus) => void,
): Promise<UnlistenFn> {
  if (!isDesktopRuntime()) {
    return () => undefined;
  }
  return listen<MonitorStatus>(MONITOR_STATE_EVENT, ({ payload }) => onEvent(payload));
}

function emptyMonitorStatus(): MonitorStatus {
  return {
    active: false,
    inputPortId: null,
    inputPortName: null,
    sessionId: null,
    error: null,
  };
}
