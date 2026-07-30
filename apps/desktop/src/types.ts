export type PortDirection = "input" | "output";

export interface PortDescriptor {
  readonly id: string;
  readonly name: string;
  readonly direction: PortDirection;
  readonly backend: string;
  readonly occurrence: number;
}

export interface PortInventory {
  readonly inputs: readonly PortDescriptor[];
  readonly outputs: readonly PortDescriptor[];
}

export interface LaboratoryStatus {
  readonly milestone: string;
  readonly parameterCount: number;
  readonly mappedBindingCount: number;
  readonly ccCount: number;
  readonly disabledCcPairCount: number;
  readonly documentedNrpnCount: number;
  readonly enabledLiveWriteCount: number;
  readonly hardwareWriteApiPresent: boolean;
  readonly exactPeakOsBuild: string | null;
  readonly hardwareGate: string;
}

export interface RawMidiEvent {
  readonly event_id: number;
  readonly monotonic_timestamp_micros: number;
  readonly wall_clock_timestamp: string;
  readonly port_id: string;
  readonly port_name: string;
  readonly direction: "input" | "output";
  readonly bytes: readonly number[];
  readonly session_id: string;
}

export type DecodedMidiMessage =
  | {
      readonly kind: "channel" | "system";
      readonly message: Record<string, unknown>;
    }
  | {
      readonly kind: "unknown";
      readonly message: {
        readonly reason: string;
      };
    };

export interface MonitorEvent {
  readonly event: RawMidiEvent;
  readonly decoded: DecodedMidiMessage;
  readonly hex: string;
  readonly decimal: readonly number[];
}

export interface MonitorStatus {
  readonly active: boolean;
  readonly inputPortId: string | null;
  readonly inputPortName: string | null;
  readonly sessionId: string | null;
  readonly error: string | null;
}
