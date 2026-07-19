use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};

use crate::keys::MAX_CONFIG_FILE_BYTES;

pub(crate) static CONFIG_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Reads the native runtime configuration. A missing file is an empty document;
/// an existing file must be a bounded, owner-only regular file whose identity
/// and length remain stable for the entire read. Malformed JSON, duplicate keys,
/// and non-object roots are surfaced instead of being interpreted as partially
/// valid configuration.
pub fn load_config(path: impl AsRef<Path>) -> io::Result<Map<String, Value>> {
    let path = path.as_ref();
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error),
    };
    validate_config_file_metadata(&before)?;

    let mut file = fs::File::open(path)?;
    let opened = file.metadata()?;
    validate_config_file_metadata(&opened)?;
    if !same_config_file(&before, &opened) {
        return Err(config_file_changed());
    }

    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or_default());
    (&mut file)
        .take(MAX_CONFIG_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_FILE_BYTES || bytes.len() as u64 != opened.len() {
        return Err(config_file_changed());
    }

    let opened_after = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    validate_config_file_metadata(&opened_after)?;
    validate_config_file_metadata(&after)?;
    if !same_config_file(&opened, &opened_after) || !same_config_file(&opened, &after) {
        return Err(config_file_changed());
    }

    let value = serde_json::from_slice::<UniqueJson>(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    value
        .0
        .as_object()
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "config root must be an object"))
}

fn validate_config_file_metadata(metadata: &fs::Metadata) -> io::Result<()> {
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config must be a regular non-symlink file",
        ));
    }
    if metadata.len() > MAX_CONFIG_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("config exceeds the {MAX_CONFIG_FILE_BYTES}-byte limit"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "config must not grant group or world permissions",
            ));
        }
    }
    Ok(())
}

fn same_config_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    if !left.file_type().is_file() || !right.file_type().is_file() || left.len() != right.len() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    true
}

fn config_file_changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "config identity or length changed while it was being read",
    )
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJson)
            .ok_or_else(|| E::custom("JSON number must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(UniqueJson(Value::String(value.to_string())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON key: {key}")));
            }
            let value = object.next_value::<UniqueJson>()?;
            values.insert(key, value.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

/// Atomically replaces the native runtime configuration while holding the same
/// sibling lock used by read/modify/write updates. Both file contents and the
/// parent-directory rename are synced before success is returned.
pub fn save_config_atomic(path: impl AsRef<Path>, config: &Map<String, Value>) -> io::Result<()> {
    let path = path.as_ref();
    with_config_lock(path, || save_config_atomic_unlocked(path, config))
}

/// Serializes a complete read/modify/write cycle across threads and processes.
/// The lock lives beside (rather than on) the config file because the final
/// atomic rename replaces the config inode.
pub fn update_config_atomic<T>(
    path: impl AsRef<Path>,
    update: impl FnOnce(&mut Map<String, Value>) -> io::Result<T>,
) -> io::Result<T> {
    let path = path.as_ref();
    with_config_lock(path, || {
        let mut config = load_config(path)?;
        let output = update(&mut config)?;
        save_config_atomic_unlocked(path, &config)?;
        Ok(output)
    })
}

fn with_config_lock<T>(path: &Path, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid config file name"))?;
    let lock_path = parent.join(format!(".{name}.lock"));
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(lock_path)?;
    lock.lock()?;
    let result = operation();
    let unlock = lock.unlock();
    match (result, unlock) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(output), Ok(())) => Ok(output),
    }
}

fn save_config_atomic_unlocked(path: &Path, config: &Map<String, Value>) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent)?;

    let mut bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    bytes.push(b'\n');

    let (temporary_path, mut temporary) = create_config_temp_file(path)?;
    if let Err(error) = temporary
        .write_all(&bytes)
        .and_then(|_| temporary.sync_all())
    {
        drop(temporary);
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    drop(temporary);

    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    fs::File::open(parent)?.sync_all()
}

fn create_config_temp_file(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid config file name"))?;

    for _ in 0..32 {
        let sequence = CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(".{name}.tmp-{}-{sequence}", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate config temporary file",
    ))
}

pub(crate) fn crc32b_hex(bytes: &[u8]) -> String {
    format!("{:08x}", crc32fast::hash(bytes))
}

/// FNV-1a (64-bit), implemented inline so the subscribe-mirror pick is
/// deterministic across processes, restarts, and platforms. `std`'s
/// `RandomState` hashing is per-process seeded and must never be used here.
pub(crate) fn fnv1a_64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}
