use super::{files, now_ms, Diagnostics};
use std::backtrace::Backtrace;
use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
thread_local! { static REPORTING: Cell<bool> = const { Cell::new(false) }; }

pub(super) fn install(service: Arc<Diagnostics>) {
    // ARCHITECTURE NOTE: never acquire the journal or engine lock here. The
    // panic may already hold it. Unique files preserve the first failure when
    // a second panic occurs during window cleanup (issue #24).
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let recursive = REPORTING
            .try_with(|flag| flag.replace(true))
            .unwrap_or(true);
        if !recursive {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let name = format!("panic-{:020}-{}-{serial}.txt", now_ms(), std::process::id());
            let path = service.directory.join(name);
            let result = files::private_file(&path, true).and_then(|mut file| {
                writeln!(
                    file,
                    "FabLabCAM {} | {} {} | time_unix_ms={} | pid={} | build={}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    now_ms(),
                    std::process::id(),
                    super::BUILD_ID
                )?;
                writeln!(file, "{info}")?;
                if service.full_backtrace.load(Ordering::Relaxed) {
                    writeln!(file, "\n{:#}", Backtrace::force_capture())?;
                }
                file.flush()?;
                file.sync_all()
            });
            match result {
                Ok(()) => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[FabLabCAM] Rapport de panne : {}",
                        path.display()
                    );
                }
                Err(e) => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[FabLabCAM] Rapport de panne non enregistré : {e}\n{info}"
                    );
                    if service.full_backtrace.load(Ordering::Relaxed) {
                        let _ = writeln!(std::io::stderr(), "{:#}", Backtrace::force_capture());
                    }
                }
            }
            let _ = REPORTING.try_with(|flag| flag.set(false));
        }
        if service.core_armed.swap(false, Ordering::AcqRel) {
            // Opt-in only. Flush above: abort does not run Rust destructors.
            std::process::abort();
        }
        previous(info);
    }));
}
