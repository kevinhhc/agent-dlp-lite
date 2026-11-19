use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn options() -> OpenOptions {
    let mut o = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    o
}

pub fn read_text(path: &Path, limit: usize) -> Result<String, String> {
    let file = options()
        .read(true)
        .open(path)
        .map_err(|_| "cannot open local file")?;
    if !file.metadata().map_err(|_| "cannot stat file")?.is_file() {
        return Err("not a regular file".into());
    }
    read_limited(file, limit)
}

pub fn read_limited(reader: impl Read, limit: usize) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read input")?;
    if bytes.len() > limit {
        return Err("input exceeds byte limit".into());
    }
    String::from_utf8(bytes).map_err(|_| "input must be UTF-8".into())
}

pub fn create(path: &Path, text: &str) -> Result<(), String> {
    let mut f = options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "cannot create file (already exists or no permission)")?;
    f.write_all(text.as_bytes())
        .and_then(|_| f.sync_all())
        .map_err(|_| "cannot write file".into())
}

pub fn stamp() -> String {
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}

pub fn replace_with_backup(path: &Path, text: &str) -> Result<Option<PathBuf>, String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("refusing symlink configuration".into());
    }
    let original = if path.exists() {
        Some(read_text(path, 1024 * 1024)?)
    } else {
        None
    };
    let backup = original.as_ref().map(|_| {
        path.with_file_name(format!(
            "{}.agent-dlp-backup-{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            stamp()
        ))
    });
    if let (Some(b), Some(old)) = (&backup, &original) {
        create(b, old)?;
    }
    let temp = path.with_file_name(format!(".agent-dlp-{}.tmp", stamp()));
    create(&temp, text)?;
    let result = (|| {
        let current = if path.exists() {
            Some(read_text(path, 1024 * 1024)?)
        } else {
            None
        };
        if current != original {
            return Err("configuration changed concurrently; retry".into());
        }
        fs::rename(&temp, path).map_err(|_| "cannot atomically replace configuration".into())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map(|_| backup)
}

pub fn audit(path: &Path, text: &str) -> Result<(), String> {
    let mut f = options()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|_| "cannot open audit log")?;
    if !f.metadata().map_err(|_| "cannot stat audit log")?.is_file() {
        return Err("audit must be a regular file".into());
    }
    f.write_all(format!("{text}\n").as_bytes())
        .map_err(|_| "cannot write audit log".into())
}
