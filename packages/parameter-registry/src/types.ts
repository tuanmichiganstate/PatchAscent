export type ParameterScope = "patch" | "global" | "runtime_clock" | "unknown";

export type ParameterBinding =
  | {
      readonly kind: "cc";
      readonly controller: number;
    }
  | {
      readonly kind: "cc_pair";
      readonly controllers: readonly [number, number];
      readonly codec: string;
    }
  | {
      readonly kind: "nrpn";
      readonly msb: number;
      readonly lsb: number;
    }
  | {
      readonly kind: "unmapped" | "unknown";
      readonly documented_control?: string;
    };

export interface ParameterEvidence {
  readonly status: string;
  readonly source_document: string;
  readonly source_page: string;
  readonly source_row_id?: string;
  readonly notes?: string;
}

export interface ParameterGates {
  readonly implementation: string;
  readonly live_write_enabled: false;
  readonly live_receive_verified: boolean;
  readonly sysex_decode_verified: boolean;
  readonly sysex_encode_verified: boolean;
}

export interface ParameterDefinition {
  readonly id: string;
  readonly label: string;
  readonly aliases?: readonly string[];
  readonly section: string;
  readonly scope: ParameterScope;
  readonly device_scope?: string;
  readonly binding: ParameterBinding;
  readonly documented_range: string;
  readonly documented_default?: string;
  readonly default_policy?: string;
  readonly display_transform?: unknown;
  readonly enum_id?: string | null;
  readonly evidence: ParameterEvidence;
  readonly gates: ParameterGates;
}

export interface RegistryMetadata {
  readonly schema_version: number;
  readonly device_profile: string;
  readonly generated_on: string;
  readonly policy: {
    readonly exact_os_build: string;
    readonly source_defaults_are_executable: false;
    readonly unknown_enum_codes_may_be_guessed: false;
    readonly cc_pair_codec_may_be_assumed_14_bit: false;
    readonly sysex_writes_enabled_by_default: false;
    readonly unknown_sysex_bytes_must_be_preserved: true;
  };
}
