//! The on-disk pipeline cache (phase 11): the compiled path-tracer pipelines are kept per adapter
//! and driver so a later Finalize does not compile again. Where the backend has no pipeline cache
//! (anything but Vulkan) everything here reports `Unsupported`.

use super::CacheOutcome;
use super::device::ComputeGpu;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn cache_dir_from_env(cadcraft: Option<&str>, xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    if let Some(dir) = cadcraft.filter(|dir| !dir.trim().is_empty()) {
        return Some(PathBuf::from(dir.trim()));
    }
    if let Some(dir) = xdg.filter(|dir| !dir.trim().is_empty()) {
        return Some(PathBuf::from(dir.trim()).join("cadcraft").join("pipelines"));
    }
    if let Some(home) = home.filter(|home| !home.trim().is_empty()) {
        return Some(PathBuf::from(home.trim()).join(".cache").join("cadcraft").join("pipelines"));
    }
    None
}

/// The cache directory: `CADCRAFT_PIPELINE_CACHE_DIR` when set (the checks use a temp dir), else
/// `$XDG_CACHE_HOME/cadcraft/pipelines`, else `$HOME/.cache/cadcraft/pipelines`; `None` when none
/// of those is set (the web build).
pub fn cache_dir() -> Option<PathBuf> {
    cache_dir_from_env(
        std::env::var("CADCRAFT_PIPELINE_CACHE_DIR").ok().as_deref(),
        std::env::var("XDG_CACHE_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

/// The cache file for `gpu`: `<dir>/pt-<adapter_key>.bin`.
pub fn cache_file(dir: &Path, gpu: &ComputeGpu) -> PathBuf {
    dir.join(format!("pt-{}.bin", gpu.adapter_key))
}

/// Open the pipeline cache for `gpu` from `dir`: `Hit` when this adapter's file was there and
/// read, `Miss` when not (an empty cache to fill), `Unsupported` without the feature or a `dir`.
pub fn open(gpu: &ComputeGpu, dir: Option<&Path>) -> (Option<wgpu::PipelineCache>, CacheOutcome) {
    let Some(dir) = dir else {
        return (None, CacheOutcome::Unsupported);
    };
    if !gpu.pipeline_cache {
        return (None, CacheOutcome::Unsupported);
    }
    if fs::create_dir_all(dir).is_err() {
        return (None, CacheOutcome::Unsupported);
    }
    let file = cache_file(dir, gpu);
    let data = match fs::read(&file) {
        Ok(bytes) if bytes.len() <= 512 * 1024 * 1024 => Some(bytes),
        Ok(_) => None,
        Err(_) => None,
    };
    let cache = cadcraft_gpu_optin::pipeline_cache(&gpu.device, "pt pipelines", data.as_deref());
    if let Some(cache) = cache {
        if data.is_some() { (Some(cache), CacheOutcome::Hit) } else { (Some(cache), CacheOutcome::Miss) }
    } else {
        (None, CacheOutcome::Unsupported)
    }
}

/// Write `cache`'s data to `dir` (atomically: a temp file renamed over the old), after the
/// pipelines were built. Errors are reported, never fatal to a render.
pub fn save(gpu: &ComputeGpu, cache: &wgpu::PipelineCache, dir: &Path) -> Result<(), String> {
    let bytes = match cache.get_data() {
        Some(data) => data,
        None => return Ok(()),
    };
    if let Err(err) = fs::create_dir_all(dir) {
        return Err(format!("create cache dir: {err}"));
    }
    let target = cache_file(dir, gpu);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let temp = dir.join(format!(".pt-{}-{}-{}.tmp", gpu.adapter_key, std::process::id(), stamp));
    fs::write(&temp, &bytes).map_err(|err| format!("write pipeline cache temp: {err}"))?;
    fs::rename(&temp, &target).map_err(|err| format!("rename pipeline cache: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn cache_dir_follows_the_env_then_xdg_then_home() {
        let _guard = ENV_LOCK.lock().unwrap();
        assert_eq!(
            cache_dir_from_env(Some("/tmp/cache-from-env"), Some("/tmp/cache-from-xdg"), Some("/tmp/home")),
            Some(PathBuf::from("/tmp/cache-from-env"))
        );
        assert_eq!(
            cache_dir_from_env(None, Some("/tmp/cache-from-xdg"), Some("/tmp/home")),
            Some(PathBuf::from("/tmp/cache-from-xdg/cadcraft/pipelines"))
        );
        assert_eq!(cache_dir_from_env(None, None, Some("/tmp/home")), Some(PathBuf::from("/tmp/home/.cache/cadcraft/pipelines")));
        assert_eq!(cache_dir_from_env(None, None, None), None);
        assert_eq!(
            cache_dir_from_env(Some(""), Some("/tmp/cache-from-xdg"), Some("/tmp/home")),
            Some(PathBuf::from("/tmp/cache-from-xdg/cadcraft/pipelines"))
        );
    }

    #[test]
    fn cache_file_is_named_by_the_adapter_key() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-cache-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let file = cache_file(&dir, &gpu);
        assert_eq!(file.file_name().and_then(|n| n.to_str()), Some(&format!("pt-{}.bin", gpu.adapter_key)[..]));
    }

    #[test]
    fn open_misses_then_hits_after_save() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.pipeline_cache {
            eprintln!("SKIP: adapter has no pipeline cache support");
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-ptcache-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let (cache, outcome) = open(&gpu, Some(dir.as_path()));
        assert_eq!(outcome, CacheOutcome::Miss);
        let cache = cache.expect("empty pipeline cache should be created when the file is absent");
        assert!(save(&gpu, &cache, &dir).is_ok());
        let bytes = std::fs::read(cache_file(&dir, &gpu)).unwrap();
        assert!(!bytes.is_empty());

        let (cache, outcome) = open(&gpu, Some(dir.as_path()));
        assert_eq!(outcome, CacheOutcome::Hit);
        assert!(cache.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unsupported_without_the_feature_or_a_dir() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-ptcache-off-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let (cache, outcome) = open(&gpu, None);
        assert_eq!(outcome, CacheOutcome::Unsupported);
        assert!(cache.is_none());

        let alt = ComputeGpu::new(gpu.device.clone(), gpu.queue.clone(), gpu.info.clone(), gpu.ray_query, false, gpu.adapter_key.clone());
        let (cache, outcome) = open(&alt, Some(dir.as_path()));
        assert_eq!(outcome, CacheOutcome::Unsupported);
        assert!(cache.is_none());
    }

    #[test]
    fn save_creates_directory_if_needed() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.pipeline_cache {
            eprintln!("SKIP: adapter has no pipeline cache support");
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-ptcache-save-mkdir-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        // dir does not exist yet; save should create it
        let (cache, _) = open(&gpu, Some(dir.as_path()));
        let cache = cache.expect("open should create the dir and return a cache");
        assert!(save(&gpu, &cache, &dir).is_ok());
        assert!(cache_file(&dir, &gpu).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_overwrites_existing_file() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.pipeline_cache {
            eprintln!("SKIP: adapter has no pipeline cache support");
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-ptcache-save-overwrite-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let (cache1, _) = open(&gpu, Some(dir.as_path()));
        let cache1 = cache1.expect("first open should succeed");
        save(&gpu, &cache1, &dir).unwrap();
        let first_size = std::fs::metadata(cache_file(&dir, &gpu)).unwrap().len();
        assert!(first_size > 0);

        // Open again (hit), save again — should overwrite cleanly
        let (cache2, outcome) = open(&gpu, Some(dir.as_path()));
        assert_eq!(outcome, CacheOutcome::Hit);
        let cache2 = cache2.expect("second open should succeed");
        save(&gpu, &cache2, &dir).unwrap();
        let second_size = std::fs::metadata(cache_file(&dir, &gpu)).unwrap().len();
        assert!(second_size > 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn open_empty_file_is_a_hit() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.pipeline_cache {
            eprintln!("SKIP: adapter has no pipeline cache support");
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-ptcache-empty-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Write an empty cache file
        let file = cache_file(&dir, &gpu);
        std::fs::write(&file, []).unwrap();

        // An existing file (even empty) is read and passed to pipeline_cache -> Hit
        let (cache, outcome) = open(&gpu, Some(dir.as_path()));
        assert_eq!(outcome, CacheOutcome::Hit);
        assert!(cache.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn open_treats_directory_as_miss() {
        let Some(gpu) = crate::gputest::shared_compute() else {
            eprintln!("SKIP: no compute device");
            return;
        };
        if !gpu.pipeline_cache {
            eprintln!("SKIP: adapter has no pipeline cache support");
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "cadcraft-ptcache-dir-miss-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Make cache_file a directory instead of a file
        let file = cache_file(&dir, &gpu);
        std::fs::create_dir(&file).unwrap();

        // fs::read on a directory returns an error -> data is None -> Miss
        let (cache, outcome) = open(&gpu, Some(dir.as_path()));
        assert_eq!(outcome, CacheOutcome::Miss);
        assert!(cache.is_some());

        let _ = std::fs::remove_dir(&file);
        let _ = std::fs::remove_dir_all(dir);
    }
}
