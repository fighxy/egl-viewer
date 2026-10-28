//! A JSON file shared by several app processes: read, change and write under a lock file,
//! replacing the file atomically (D-022).

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use library_core::SourceError;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) struct JsonFile {
    path: PathBuf,
}

impl JsonFile {
    pub(crate) fn new(dir: &Path, name: &str) -> Self {
        Self {
            path: dir.join(name),
        }
    }

    /// The stored value, or the default when the file is missing or unreadable.
    pub(crate) fn load<T: DeserializeOwned + Default>(&self) -> T {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(
                    "{} is unreadable, using defaults: {error}",
                    self.path.display()
                );
                T::default()
            }),
            Err(error) if error.kind() == ErrorKind::NotFound => T::default(),
            Err(error) => {
                tracing::warn!("cannot read {}: {error}", self.path.display());
                T::default()
            }
        }
    }

    /// Re-reads the file, applies `change` and writes the result, all under the lock.
    pub(crate) fn update<T, R>(&self, change: impl FnOnce(&mut T) -> R) -> Result<R, SourceError>
    where
        T: Serialize + DeserializeOwned + Default,
    {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir).map_err(storage("create the settings folder"))?;
        let lock = self.lock()?;
        let mut value: T = self.load();
        let result = change(&mut value);
        self.write(&value)?;
        drop(lock);
        Ok(result)
    }

    fn lock(&self) -> Result<File, SourceError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_extension("lock"))
            .map_err(storage("open the settings lock"))?;
        file.lock().map_err(storage("lock the settings"))?;
        Ok(file)
    }

    fn write<T: Serialize>(&self, value: &T) -> Result<(), SourceError> {
        let tmp = self
            .path
            .with_extension(format!("tmp-{}", std::process::id()));
        let bytes = serde_json::to_vec_pretty(value).map_err(storage("encode the settings"))?;
        let mut file = File::create(&tmp).map_err(storage("write the settings"))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(storage("write the settings"))?;
        drop(file);
        fs::rename(&tmp, &self.path).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            SourceError::Storage(format!("replace the settings: {error}"))
        })
    }
}

fn storage<E: std::fmt::Display>(what: &'static str) -> impl Fn(E) -> SourceError {
    move |error| SourceError::Storage(format!("{what}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Counter {
        values: Vec<u32>,
    }

    #[test]
    fn updates_merge_instead_of_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let first = JsonFile::new(dir.path(), "counter.json");
        let second = JsonFile::new(dir.path(), "counter.json");
        first.update(|c: &mut Counter| c.values.push(1)).unwrap();
        second.update(|c: &mut Counter| c.values.push(2)).unwrap();
        assert_eq!(first.load::<Counter>().values, vec![1, 2]);
    }

    #[test]
    fn concurrent_writers_lose_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let path = path.clone();
                std::thread::spawn(move || {
                    JsonFile::new(&path, "counter.json")
                        .update(|c: &mut Counter| c.values.push(n))
                        .unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let mut values = JsonFile::new(&path, "counter.json")
            .load::<Counter>()
            .values;
        values.sort_unstable();
        assert_eq!(values, (0..8).collect::<Vec<_>>());
    }

    #[test]
    fn a_broken_file_reads_as_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("counter.json"), "{ not json").unwrap();
        let file = JsonFile::new(dir.path(), "counter.json");
        assert_eq!(file.load::<Counter>(), Counter::default());
    }
}
