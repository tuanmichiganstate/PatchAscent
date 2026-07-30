import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";

vi.mock("./lib/tauri", () => {
  const monitorStatus = {
    active: false,
    inputPortId: null,
    inputPortName: null,
    sessionId: null,
    error: null,
  };
  return {
    isDesktopRuntime: () => false,
    getLaboratoryStatus: () => Promise.resolve({
      milestone: "M0 · Protocol Laboratory",
      parameterCount: 251,
      mappedBindingCount: 250,
      ccCount: 56,
      disabledCcPairCount: 18,
      documentedNrpnCount: 176,
      enabledLiveWriteCount: 0,
      hardwareWriteApiPresent: false,
      exactPeakOsBuild: null,
      hardwareGate: "HV-001 through HV-014 pending physical Peak",
    }),
    getMidiPorts: () => Promise.resolve({ inputs: [], outputs: [] }),
    getMonitorStatus: () => Promise.resolve(monitorStatus),
    startMidiMonitor: () => Promise.resolve(monitorStatus),
    stopMidiMonitor: () => Promise.resolve(monitorStatus),
    listenForRawMidi: () => Promise.resolve(() => undefined),
    listenForMonitorState: () => Promise.resolve(() => undefined),
  };
});

describe("PatchAscent diagnostics shell", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows the Milestone 0 safety boundary", async () => {
    render(<App />);

    expect(await screen.findByText("PatchAscent")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", {
        name: "Listen to the instrument. Keep every byte.",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("Stored-memory write API")).toBeInTheDocument();
    expect(screen.getByText("Not present")).toBeInTheDocument();
    expect(screen.getByText("CC-pair writes disabled")).toBeInTheDocument();
  });

  it("does not expose a raw send control", () => {
    render(<App />);
    const buttons = screen.getAllByRole("button").map((button) => button.textContent);
    expect(buttons.join(" ")).not.toMatch(/send|write|store/i);
  });
});
