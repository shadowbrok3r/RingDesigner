//! Keep saved designs and layouts at one location across desktop releases.
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "RingDesigner Desktop";

pub fn persistence_path() -> Option<PathBuf> {
    let destination = eframe::storage_dir(APP_ID)?.join("app.ron");
    let names = [
        format!("RingDesigner v{}", env!("CARGO_PKG_VERSION")),
        "RingDesigner v0.3.0".into(),
        "RingDesigner v0.2.0".into(),
        "RingDesigner v0.1.0".into(),
        "RingDesigner".into(),
    ];
    let candidates = names
        .iter()
        .filter_map(|name| eframe::storage_dir(name).map(|dir| dir.join("app.ron")))
        .collect::<Vec<_>>();
    Some(prepare(&destination, &candidates))
}

fn prepare(destination: &Path, candidates: &[PathBuf]) -> PathBuf {
    if destination.exists() {
        return destination.into();
    }
    let source = candidates
        .iter()
        .filter_map(|path| {
            let metadata = path.metadata().ok()?;
            if !metadata.is_file() || metadata.len() == 0 { return None; }
            Some((metadata.modified().ok()?, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path);
    let Some(source) = source else {
        return destination.into();
    };
    match copy_once(source, destination) {
        Ok(()) => destination.into(),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && destination.is_file() => destination.into(),
        Err(error) => {
            // Continue saving to the old path if migration is unavailable.
            log::warn!(
                "Session migration failed; keeping {}: {error}",
                source.display()
            );
            source.clone()
        }
    }
}

fn copy_once(source: &Path, destination: &Path) -> std::io::Result<()> {
    let parent = destination.parent().expect("session file has a parent");
    std::fs::create_dir_all(parent)?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::copy(source, temporary.path())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrating_preserves_legacy_and_never_overwrites_an_existing_session() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old.ron");
        let destination = temp.path().join("stable/app.ron");
        std::fs::write(&old, "unsaved design and resized graph workspace").unwrap();
        assert_eq!(
            prepare(&destination, std::slice::from_ref(&old)),
            destination
        );
        assert_eq!(
            std::fs::read(&old).unwrap(),
            std::fs::read(&destination).unwrap()
        );
        std::fs::write(&destination, "newer stable session").unwrap();
        prepare(&destination, &[old]);
        assert_eq!(
            std::fs::read_to_string(&destination).unwrap(),
            "newer stable session"
        );
    }
    #[test]
    fn failed_migration_keeps_the_original_storage_path() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old.ron");
        let blocked = temp.path().join("not-a-directory");
        std::fs::write(&old, "saved session").unwrap();
        std::fs::write(&blocked, "a file").unwrap();
        assert_eq!(
            prepare(&blocked.join("app.ron"), std::slice::from_ref(&old)),
            old
        );
    }
}
