// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! The Kshana Pro service under the Windows Service Control Manager (roadmap L5).
//!
//! `kshana-pro serve --windows-service --config <serve.conf>` is the command the MSI
//! registers for the `kshana-pro` service. It connects to the Service Control Manager,
//! reports "running" once the server is listening, and on Stop (or system shutdown)
//! stops as [`ServerHandle::stop`](super::ServerHandle::stop) does: no new connections,
//! running jobs finish, queued jobs stay on disk for the next start.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

use super::ServeConfig;

/// The service name the MSI installs.
pub const SERVICE_NAME: &str = "kshana-pro";

/// The configuration and log path handed to the service thread (the dispatcher calls
/// `service_main` with no way to pass them).
static PENDING: Mutex<Option<(ServeConfig, Option<PathBuf>)>> = Mutex::new(None);

define_windows_service!(ffi_service_main, service_main);

/// Run `config` as the `kshana-pro` Windows service until the Service Control Manager
/// stops it. Errors are also appended to `log`, since a service has no console.
pub fn run(config: ServeConfig, log: Option<PathBuf>) -> Result<(), String> {
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = Some((config, log));
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).map_err(|e| {
        format!("not started by the Service Control Manager ({e}); start it with: Start-Service {SERVICE_NAME}")
    })
}

fn service_main(_args: Vec<OsString>) {
    let Some((config, log)) = PENDING.lock().unwrap_or_else(|p| p.into_inner()).take() else {
        return;
    };
    if let Err(e) = run_service(config) {
        if let Some(p) = log {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
            {
                let _ = writeln!(f, "kshana-pro service: {e}");
            }
        }
    }
}

fn status(state: ServiceState, exit: u32, wait: Duration) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running {
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
        } else {
            ServiceControlAccept::empty()
        },
        exit_code: ServiceExitCode::Win32(exit),
        checkpoint: 0,
        wait_hint: wait,
        process_id: None,
    }
}

fn run_service(config: ServeConfig) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let handle = service_control_handler::register(SERVICE_NAME, move |c| match c {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = tx.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })
    .map_err(|e| format!("could not register with the Service Control Manager: {e}"))?;
    let set = |s: ServiceStatus| handle.set_service_status(s).map_err(|e| e.to_string());
    set(status(
        ServiceState::StartPending,
        0,
        Duration::from_secs(30),
    ))?;
    let addr = config.addr.clone();
    let server = match super::start(config) {
        Ok(s) => s,
        Err(e) => {
            // ERROR_SERVICE_SPECIFIC_ERROR would hide the reason; 1 is a plain failure,
            // and the reason is in service.log.
            let _ = set(status(ServiceState::Stopped, 1, Duration::ZERO));
            return Err(format!("could not serve on {addr}: {e}"));
        }
    };
    set(status(ServiceState::Running, 0, Duration::ZERO))?;
    let _ = rx.recv();
    // Running jobs finish before the process ends; tell the manager it may take a while.
    set(status(
        ServiceState::StopPending,
        0,
        Duration::from_secs(120),
    ))?;
    server.stop();
    set(status(ServiceState::Stopped, 0, Duration::ZERO))?;
    Ok(())
}
