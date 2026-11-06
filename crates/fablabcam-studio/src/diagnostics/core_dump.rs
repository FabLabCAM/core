//! Core capture is opt-in for the current process. Never change a sysctl,
//! launchd policy, security entitlement, or another process's limits.

// XNU compares RLIMIT_CORE against the virtual map, including large reserved
// regions. A small disk-sized limit silently prevents dumps on Apple Silicon.
// This finite virtual ceiling is NOT a disk budget (ADR 0006).
pub const CORE_LIMIT: u64 = 1024 * 1024 * 1024 * 1024;
#[cfg(target_os = "macos")]
const MIN_FREE_BYTES: u64 = 10 * 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct CoreStatus {
    pub available: bool,
    pub summary: String,
    pub destination: String,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{CoreStatus, CORE_LIMIT, MIN_FREE_BYTES};
    use std::ffi::CString;
    use std::path::Path;

    pub type SavedLimit = libc::rlimit;

    fn check_signature() -> Result<(), String> {
        // Public XNU codesign.h: CS_OPS_STATUS=0, CS_GET_TASK_ALLOW=0x4.
        extern "C" {
            fn csops(pid: libc::pid_t, ops: u32, address: *mut libc::c_void, size: usize) -> i32;
        }
        let mut flags: u32 = 0;
        // SAFETY: query our own process into a correctly sized writable u32.
        if unsafe {
            csops(
                std::process::id() as libc::pid_t,
                0,
                (&mut flags as *mut u32).cast(),
                std::mem::size_of::<u32>(),
            )
        } != 0
        {
            return Err(format!(
                "Signature de diagnostic invérifiable : {}",
                std::io::Error::last_os_error()
            ));
        }
        if flags & 0x4 == 0 {
            return Err("Ce binaire n'autorise pas les core dumps. Utilisez la compilation de diagnostic macOS (get-task-allow) ; les rapports texte restent disponibles.".into());
        }
        Ok(())
    }

    fn sysctl(name: &str) -> Result<Vec<u8>, String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let mut size = 0;
        // SAFETY: name is NUL terminated, len points to initialized storage;
        // sysctl only writes into our sized allocation on the second call.
        unsafe {
            if libc::sysctlbyname(
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut bytes = vec![0; size];
            if libc::sysctlbyname(
                name.as_ptr(),
                bytes.as_mut_ptr().cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            bytes.truncate(size);
            Ok(bytes)
        }
    }

    fn limits() -> Result<libc::rlimit, String> {
        let mut value = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: getrlimit initializes a valid, writable rlimit structure.
        if unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut value) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(value)
    }

    fn available_bytes(directory: &Path) -> Result<u64, String> {
        use std::os::unix::ffi::OsStrExt;
        let name = CString::new(directory.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: initialized only after a successful statvfs call. access only
        // inspects the NUL-terminated directory path.
        unsafe {
            if libc::access(name.as_ptr(), libc::W_OK) != 0
                || libc::statvfs(name.as_ptr(), stat.as_mut_ptr()) != 0
            {
                return Err(format!(
                    "Dossier core inaccessible : {}",
                    std::io::Error::last_os_error()
                ));
            }
            let stat = stat.assume_init();
            Ok(u64::from(stat.f_bavail).saturating_mul(stat.f_frsize))
        }
    }

    fn inspect() -> Result<(String, libc::rlimit, u64), String> {
        check_signature()?;
        let enabled = sysctl("kern.coredump")?;
        if enabled
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .map(i32::from_ne_bytes)
            != Some(1)
        {
            return Err("Les core dumps sont désactivés par macOS (kern.coredump)".into());
        }
        let template = String::from_utf8(sysctl("kern.corefile")?).map_err(|e| e.to_string())?;
        let template = template.trim_end_matches('\0');
        // Expand the documented process token only. Report unsupported system
        // templates rather than probing/writing a guessed destination.
        let expanded = template.replace("%P", &std::process::id().to_string());
        if expanded.contains('%') || !Path::new(&expanded).is_absolute() {
            return Err(format!(
                "Chemin core système non pris en charge : {template}"
            ));
        }
        if Path::new(&expanded)
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Err(format!(
                "Un fichier core existe déjà à cette destination : {expanded}"
            ));
        }
        let directory = Path::new(&expanded).parent().ok_or("Dossier core absent")?;
        let limits = limits()?;
        if limits.rlim_max < CORE_LIMIT {
            return Err("Limite dure RLIMIT_CORE insuffisante (1 Tio virtuel nécessaire)".into());
        }
        let free = available_bytes(directory)?;
        if free < MIN_FREE_BYTES {
            return Err("Espace disque insuffisant pour le core (10 Gio libres requis)".into());
        }
        Ok((expanded, limits, free))
    }

    pub fn status() -> CoreStatus {
        match inspect() {
            Ok((destination, limits, free)) => CoreStatus {
                available: true,
                summary: format!("Préconditions noyau/disque vérifiées ; limite actuelle {} octets ; {} Mio libres. Limite virtuelle : 1 Tio. Le fichier peut occuper plusieurs Gio ; aucun plafond disque strict n’est garanti.", limits.rlim_cur, free / 1024 / 1024),
                destination,
            },
            Err(summary) => CoreStatus { available: false, summary, destination: String::new() },
        }
    }

    pub fn arm() -> Result<SavedLimit, String> {
        let (_, previous, _) = inspect()?;
        let next = libc::rlimit {
            rlim_cur: CORE_LIMIT,
            rlim_max: previous.rlim_max,
        };
        // SAFETY: only this process's soft limit changes, within its hard limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &next) } != 0 {
            return Err(format!(
                "Activation du core refusée : {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(previous)
    }

    pub fn restore(previous: &SavedLimit) -> Result<(), String> {
        // Preserve the current hard limit if another component changed it.
        let current = limits()?;
        let next = libc::rlimit {
            rlim_cur: previous.rlim_cur.min(current.rlim_max),
            rlim_max: current.rlim_max,
        };
        // SAFETY: the value is bounded by the current hard limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &next) } != 0 {
            return Err(format!(
                "Restauration de la limite core : {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::CoreStatus;
    pub type SavedLimit = ();
    pub fn status() -> CoreStatus {
        CoreStatus { available: false, summary: "Capture core intégrée disponible sur macOS uniquement. Les rapports texte restent disponibles.".into(), destination: String::new() }
    }
    pub fn arm() -> Result<SavedLimit, String> {
        Err(status().summary)
    }
    pub fn restore(_: &SavedLimit) -> Result<(), String> {
        Ok(())
    }
}

pub use platform::{arm, restore, status, SavedLimit};
