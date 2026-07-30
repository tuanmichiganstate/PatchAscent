use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use parking_lot::Mutex;
use patchascent_midi_messages::{format_hex, DecodedMidiMessage, RawMidiEvent};
use patchascent_midi_transport::{MidiBackend, MidirBackend, PortInventory};
use patchascent_peak_domain::{Binding, ParameterRegistry};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const RAW_EVENT_NAME: &str = "patchascent://raw-midi";
const MONITOR_STATE_EVENT_NAME: &str = "patchascent://monitor-state";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaboratoryStatus {
    milestone: &'static str,
    parameter_count: usize,
    mapped_binding_count: usize,
    cc_count: usize,
    disabled_cc_pair_count: usize,
    documented_nrpn_count: usize,
    enabled_live_write_count: usize,
    hardware_write_api_present: bool,
    exact_peak_os_build: Option<String>,
    hardware_gate: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorEvent {
    event: RawMidiEvent,
    decoded: DecodedMidiMessage,
    hex: String,
    decimal: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorStatus {
    active: bool,
    input_port_id: Option<String>,
    input_port_name: Option<String>,
    session_id: Option<Uuid>,
    error: Option<String>,
}

struct MonitorHandle {
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    input_port_id: String,
    input_port_name: String,
    session_id: Uuid,
    error: Arc<Mutex<Option<String>>>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for MonitorHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MonitorHandle")
            .field("input_port_id", &self.input_port_id)
            .field("input_port_name", &self.input_port_name)
            .field("session_id", &self.session_id)
            .field("active", &self.active.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct MonitorManager {
    current: Mutex<Option<MonitorHandle>>,
}

impl MonitorManager {
    fn stop(&self) -> MonitorStatus {
        let mut current = self.current.lock();
        let Some(mut handle) = current.take() else {
            return MonitorStatus {
                active: false,
                input_port_id: None,
                input_port_name: None,
                session_id: None,
                error: None,
            };
        };
        handle.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = handle.worker.take() {
            let _ = worker.join();
        }
        let error = handle.error.lock().clone();
        MonitorStatus {
            active: false,
            input_port_id: Some(handle.input_port_id),
            input_port_name: Some(handle.input_port_name),
            session_id: Some(handle.session_id),
            error,
        }
    }

    fn status(&self) -> MonitorStatus {
        let current = self.current.lock();
        let Some(handle) = current.as_ref() else {
            return MonitorStatus {
                active: false,
                input_port_id: None,
                input_port_name: None,
                session_id: None,
                error: None,
            };
        };
        let error = handle.error.lock().clone();
        MonitorStatus {
            active: handle.active.load(Ordering::Relaxed),
            input_port_id: Some(handle.input_port_id.clone()),
            input_port_name: Some(handle.input_port_name.clone()),
            session_id: Some(handle.session_id),
            error,
        }
    }
}

impl Drop for MonitorManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[tauri::command]
fn laboratory_status() -> Result<LaboratoryStatus, String> {
    let registry = ParameterRegistry::embedded().map_err(|error| error.to_string())?;
    registry
        .validate_seed_safety()
        .map_err(|error| error.to_string())?;
    let mut cc_count = 0;
    let mut cc_pair_count = 0;
    let mut nrpn_count = 0;
    for parameter in &registry.parameters {
        match parameter.binding {
            Binding::Cc { .. } => cc_count += 1,
            Binding::CcPair { .. } => cc_pair_count += 1,
            Binding::Nrpn { .. } => nrpn_count += 1,
            Binding::Unmapped | Binding::Unknown => {}
        }
    }
    Ok(LaboratoryStatus {
        milestone: "M0 · Protocol Laboratory",
        parameter_count: registry.parameters.len(),
        mapped_binding_count: registry.binding_index().len(),
        cc_count,
        disabled_cc_pair_count: cc_pair_count,
        documented_nrpn_count: nrpn_count,
        enabled_live_write_count: registry
            .parameters
            .iter()
            .filter(|parameter| parameter.gates.live_write_enabled)
            .count(),
        hardware_write_api_present: false,
        exact_peak_os_build: None,
        hardware_gate: "HV-001 through HV-014 pending physical Peak",
    })
}

#[tauri::command]
fn list_midi_ports() -> Result<PortInventory, String> {
    MidirBackend.list_ports().map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects AppHandle and State command arguments by value"
)]
fn start_monitor(
    app: AppHandle,
    manager: State<'_, MonitorManager>,
    input_port_id: String,
) -> Result<MonitorStatus, String> {
    let _ = manager.stop();
    let session_id = Uuid::new_v4();
    let session = MidirBackend
        .open_input(&input_port_id, session_id, 8192)
        .map_err(|error| error.to_string())?;
    let input_port_name = session.descriptor().name.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let active = Arc::new(AtomicBool::new(true));
    let error = Arc::new(Mutex::new(None));
    let stop_for_worker = Arc::clone(&stop);
    let active_for_worker = Arc::clone(&active);
    let error_for_worker = Arc::clone(&error);
    let app_for_worker = app.clone();

    let worker = thread::Builder::new()
        .name("patchascent-midi-monitor".to_owned())
        .spawn(move || {
            while !stop_for_worker.load(Ordering::Relaxed) {
                match session.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(event)) => {
                        let payload = MonitorEvent {
                            hex: format_hex(&event.bytes),
                            decimal: event.bytes.clone(),
                            decoded: event.decoded(),
                            event,
                        };
                        let _ = app_for_worker.emit(RAW_EVENT_NAME, payload);
                    }
                    Ok(None) => {}
                    Err(transport_error) => {
                        *error_for_worker.lock() = Some(transport_error.to_string());
                        break;
                    }
                }
            }
            let dropped = session.dropped_count();
            session.close();
            active_for_worker.store(false, Ordering::Relaxed);
            let status = MonitorStatus {
                active: false,
                input_port_id: None,
                input_port_name: None,
                session_id: Some(session_id),
                error: error_for_worker
                    .lock()
                    .clone()
                    .or_else(|| (dropped > 0).then(|| format!("{dropped} raw events dropped"))),
            };
            let _ = app_for_worker.emit(MONITOR_STATE_EVENT_NAME, status);
        })
        .map_err(|error| error.to_string())?;

    *manager.current.lock() = Some(MonitorHandle {
        stop,
        active,
        input_port_id,
        input_port_name,
        session_id,
        error,
        worker: Some(worker),
    });
    Ok(manager.status())
}

#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects State command arguments by value"
)]
fn stop_monitor(manager: State<'_, MonitorManager>) -> MonitorStatus {
    manager.stop()
}

#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects State command arguments by value"
)]
fn monitor_status(manager: State<'_, MonitorManager>) -> MonitorStatus {
    manager.status()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(MonitorManager::default())
        .invoke_handler(tauri::generate_handler![
            laboratory_status,
            list_midi_ports,
            start_monitor,
            stop_monitor,
            monitor_status
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("PatchAscent · Protocol Laboratory");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PatchAscent");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_exposes_no_write_path() {
        let status = laboratory_status().unwrap();
        assert_eq!(status.parameter_count, 251);
        assert_eq!(status.enabled_live_write_count, 0);
        assert!(!status.hardware_write_api_present);
        assert!(status.disabled_cc_pair_count > 0);
    }
}
