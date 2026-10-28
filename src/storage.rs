use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const O_NOFOLLOW: i32 = 0o400000;
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;
const CONFIG_LIMIT: u64 = 64 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub keep_awake: [bool; 5],
    pub fit_whatsapp: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keep_awake: [false; 5],
            fit_whatsapp: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Storage {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub icons_dir: PathBuf,
}

impl Storage {
    pub fn prepare(data_override: Option<&Path>) -> io::Result<Self> {
        let data = match data_override {
            Some(path) => absolute(path)?,
            None => xdg_base("XDG_DATA_HOME", ".local/share")?.join("relay-native"),
        };
        // Firefox profiles use a different format. Never repurpose an old profile.
        if ["prefs.js", "extensions.json", "compatibility.ini"]
            .iter()
            .any(|name| data.join(name).exists())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Choose a separate native app data directory.",
            ));
        }
        let cache = match data_override {
            Some(_) => data.join("cache"),
            None => xdg_base("XDG_CACHE_HOME", ".cache")?.join("relay-native"),
        };
        private_directory(&data)?;
        private_directory(&cache)?;
        let data_dir = data.canonicalize()?;
        let cache_dir = cache.canonicalize()?;
        let icons_dir = data_dir.join("icons");
        private_directory(&icons_dir)?;
        for (name, contents) in [
            (
                "whatsapp.svg",
                include_bytes!("../icons/whatsapp.svg").as_slice(),
            ),
            (
                "telegram.svg",
                include_bytes!("../icons/telegram.svg").as_slice(),
            ),
            (
                "instagram.svg",
                include_bytes!("../icons/instagram.svg").as_slice(),
            ),
            (
                "linkedin.svg",
                include_bytes!("../icons/linkedin.svg").as_slice(),
            ),
            ("gmail.svg", include_bytes!("../icons/gmail.svg").as_slice()),
        ] {
            let destination = icons_dir.join(name);
            match read_private_file(&destination, CONFIG_LIMIT) {
                Ok(existing) if existing == contents => {}
                Ok(_) => write_private_atomically(&destination, contents)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    write_private_atomically(&destination, contents)?
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Self {
            data_dir,
            cache_dir,
            icons_dir,
        })
    }

    pub fn load_config(&self) -> io::Result<Config> {
        match read_private_file(&self.data_dir.join("config.json"), CONFIG_LIMIT) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save_config(&self, config: &Config) -> io::Result<()> {
        let bytes = serde_json::to_vec(config).map_err(io::Error::other)?;
        write_private_atomically(&self.data_dir.join("config.json"), &bytes)
    }
}

/// A nonblocking advisory lock held only by the visible application's process.
/// Closing the file releases it, so a crash cannot leave a stale process marker.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid app lock file.",
            ));
        }
        file.set_permissions(Permissions::from_mode(0o600))?;
        if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { _file: file })
    }
}

fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Data directory is empty.",
        ));
    }
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        env::current_dir()?.join(path)
    })
}

fn xdg_base(variable: &str, home_suffix: &str) -> io::Result<PathBuf> {
    if let Some(value) = env::var_os(variable).filter(|value| Path::new(value).is_absolute()) {
        return Ok(PathBuf::from(value));
    }
    let home: OsString = env::var_os("HOME")
        .filter(|value| Path::new(value).is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME directory is unavailable."))?;
    Ok(PathBuf::from(home).join(home_suffix))
}

fn reject_symlink_components(path: &Path) -> io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "App directories cannot be symbolic links.",
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> io::Result<()> {
    reject_symlink_components(path)?;
    fs::create_dir_all(path)?;
    reject_symlink_components(path)?;
    if !fs::metadata(path)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "App path is not a directory.",
        ));
    }
    if fs::metadata(path)?.permissions().mode() & 0o777 != 0o700 {
        fs::set_permissions(path, Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn read_private_file(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid app data file.",
        ));
    }
    if metadata.permissions().mode() & 0o777 != 0o600 {
        file.set_permissions(Permissions::from_mode(0o600))?;
    }
    let mut contents = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut contents)?;
    Ok(contents)
}

fn write_private_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "App data cannot be a symbolic link or directory.",
            ))
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid app data path."))?;
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".relay-{}-{sequence}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("relay-test-{}-{sequence}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn directories_and_saved_config_are_private_and_round_trip() {
        let temp = TestDirectory::new();
        let storage = Storage::prepare(Some(&temp.0.join("app"))).unwrap();
        for directory in [&storage.data_dir, &storage.cache_dir, &storage.icons_dir] {
            assert_eq!(
                fs::metadata(directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(storage.load_config().unwrap(), Config::default());
        let config = Config {
            keep_awake: [true, false, false, true, false],
            ..Config::default()
        };
        storage.save_config(&config).unwrap();
        assert_eq!(storage.load_config().unwrap(), config);
        assert_eq!(
            fs::metadata(storage.data_dir.join("config.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(storage.icons_dir.join("whatsapp.svg").is_file());
        assert!(!fs::read_dir(&storage.data_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn legacy_config_enables_whatsapp_fit_and_opt_out_round_trips() {
        let temp = TestDirectory::new();
        let storage = Storage::prepare(Some(&temp.0.join("app"))).unwrap();
        fs::write(
            storage.data_dir.join("config.json"),
            br#"{"keep_awake":[true,false,true,false,false]}"#,
        )
        .unwrap();
        let mut config = storage.load_config().unwrap();
        assert_eq!(config.keep_awake, [true, false, true, false, false]);
        assert!(config.fit_whatsapp);

        config.fit_whatsapp = false;
        storage.save_config(&config).unwrap();
        assert_eq!(storage.load_config().unwrap(), config);
        assert!(!storage.load_config().unwrap().fit_whatsapp);
    }

    #[test]
    fn corrupt_config_resets_preferences_without_loading_credentials() {
        let temp = TestDirectory::new();
        let storage = Storage::prepare(Some(&temp.0.join("app"))).unwrap();
        fs::write(storage.data_dir.join("config.json"), b"{broken json").unwrap();
        assert_eq!(storage.load_config().unwrap(), Config::default());
    }

    #[test]
    fn single_instance_lock_rejects_another_owner_and_releases_on_drop() {
        let temp = TestDirectory::new();
        let path = temp.0.join("relay.lock");
        let first = InstanceLock::acquire(&path).unwrap();
        assert_eq!(
            InstanceLock::acquire(&path).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(first);
        assert!(InstanceLock::acquire(&path).is_ok());
    }

    #[test]
    fn data_directories_config_and_lock_reject_symbolic_links() {
        let temp = TestDirectory::new();
        fs::create_dir(temp.0.join("target")).unwrap();
        symlink(temp.0.join("target"), temp.0.join("linked")).unwrap();
        assert!(Storage::prepare(Some(&temp.0.join("linked"))).is_err());
        let storage = Storage::prepare(Some(&temp.0.join("app"))).unwrap();
        let target = temp.0.join("outside");
        fs::write(&target, b"untouched").unwrap();
        symlink(&target, storage.data_dir.join("config.json")).unwrap();
        assert!(storage.load_config().is_err());
        assert!(storage.save_config(&Config::default()).is_err());
        symlink(&target, storage.data_dir.join("relay.lock")).unwrap();
        assert!(InstanceLock::acquire(&storage.data_dir.join("relay.lock")).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"untouched");
    }

    #[test]
    fn firefox_profile_is_never_reused_as_webkit_storage() {
        let temp = TestDirectory::new();
        fs::write(temp.0.join("prefs.js"), b"old Firefox profile").unwrap();
        assert!(Storage::prepare(Some(&temp.0)).is_err());
        assert_eq!(
            fs::read(temp.0.join("prefs.js")).unwrap(),
            b"old Firefox profile"
        );
    }
}
