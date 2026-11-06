use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const MAX_LOG_BYTES: u64 = 1_048_576;
const ROTATIONS: usize = 3;
const MAX_PANICS: usize = 20;
const JOURNAL_SLOTS: usize = 8;

pub fn prepare_directory(directory: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
        builder.create(directory)?;
        // An override may point at an existing directory: never chmod it.
    }
    #[cfg(not(unix))]
    builder.create(directory)?;
    Ok(())
}

pub fn private_file(path: &Path, exclusive: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).append(true).read(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

pub struct Journal {
    directory: PathBuf,
    file: File,
    bytes: u64,
    prefix: String,
    _lock: File,
}

impl Journal {
    pub fn open(directory: &Path) -> io::Result<Self> {
        // Each live instance owns a fixed slot. OS locks are released even on
        // abort; rotations never rename another instance's active journal.
        for slot in 0..JOURNAL_SLOTS {
            let prefix = format!("session-{slot}");
            let lock = private_file(&directory.join(format!("{prefix}.lock")), false)?;
            match lock.try_lock() {
                Ok(()) => {
                    let file = private_file(&directory.join(format!("{prefix}.jsonl")), false)?;
                    let bytes = file.metadata()?.len();
                    return Ok(Self {
                        directory: directory.to_owned(),
                        file,
                        bytes,
                        prefix,
                        _lock: lock,
                    });
                }
                Err(std::fs::TryLockError::WouldBlock) => continue,
                Err(std::fs::TryLockError::Error(error)) => return Err(error),
            }
        }
        Err(io::Error::other(
            "Les huit emplacements de journal sont utilisés",
        ))
    }

    pub fn write(&mut self, value: &serde_json::Value) -> io::Result<()> {
        let mut data = serde_json::to_vec(value)?;
        data.push(b'\n');
        if data.len() as u64 > MAX_LOG_BYTES {
            return Err(io::Error::other("Événement de diagnostic trop volumineux"));
        }
        if self.bytes + data.len() as u64 > MAX_LOG_BYTES {
            self.rotate()?;
        }
        self.file.write_all(&data)?;
        self.file.flush()?;
        self.bytes += data.len() as u64;
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        // Close before renaming, including on Windows. The temporary file is
        // unlinked on drop and is never included in a diagnostic export.
        self.file = tempfile::tempfile()?;
        let oldest = self
            .directory
            .join(format!("{}.{ROTATIONS}.jsonl", self.prefix));
        if oldest.exists() {
            std::fs::remove_file(oldest)?;
        }
        for n in (1..ROTATIONS).rev() {
            let from = self.directory.join(format!("{}.{n}.jsonl", self.prefix));
            if from.exists() {
                std::fs::rename(
                    from,
                    self.directory
                        .join(format!("{}.{}.jsonl", self.prefix, n + 1)),
                )?;
            }
        }
        std::fs::rename(
            self.directory.join(format!("{}.jsonl", self.prefix)),
            self.directory.join(format!("{}.1.jsonl", self.prefix)),
        )?;
        self.file = private_file(
            &self.directory.join(format!("{}.jsonl", self.prefix)),
            false,
        )?;
        self.bytes = 0;
        Ok(())
    }
}

pub fn report_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let journal = (0..JOURNAL_SLOTS).any(|slot| {
            name == format!("session-{slot}.jsonl")
                || (1..=ROTATIONS).any(|n| name == format!("session-{slot}.{n}.jsonl"))
        });
        let panic = name.starts_with("panic-") && name.ends_with(".txt");
        if journal || panic {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

/// Retention happens at startup, never while handling a panic.
pub fn prune_panics(directory: &Path) -> io::Result<()> {
    let paths: Vec<_> = report_files(directory)?
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("panic-"))
        })
        .collect();
    for path in paths.iter().take(paths.len().saturating_sub(MAX_PANICS)) {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// A text bundle deliberately excludes settings, projects, credentials and cores.
/// Persist atomically so a failed export does not destroy an existing report.
pub fn export(directory: &Path, destination: &Path) -> Result<(), String> {
    let paths = report_files(directory).map_err(|e| e.to_string())?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if parent.canonicalize().ok() == directory.canonicalize().ok() {
        return Err("Choisissez un dossier différent de celui des diagnostics".into());
    }
    let mut out = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    writeln!(
        out,
        "FabLabCAM — rapports texte uniquement ; aucun dump mémoire"
    )
    .map_err(|e| e.to_string())?;
    for path in paths {
        let mut input = File::open(&path).map_err(|e| e.to_string())?;
        // Bound exports even if an unrelated process replaced a matching file.
        if input.metadata().map_err(|e| e.to_string())?.len() > 16 * MAX_LOG_BYTES {
            return Err("Rapport trop volumineux pour l'export texte".into());
        }
        writeln!(
            out,
            "\n--- {} ---",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
        .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut out).map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    out.as_file().sync_all().map_err(|e| e.to_string())?;
    out.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
}
