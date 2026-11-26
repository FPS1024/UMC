//! Vanilla installation and launch primitives shared by every adapter.
use futures_util::{StreamExt, TryStreamExt, stream};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use umc_api::{
    GameProcess, INSTANCE_CONFIG_SCHEMA_VERSION, InstanceConfig, JavaRuntimeInfo,
    JavaRuntimeInstallResult, MemoryConfig, ProcessKind, ProcessStatus, RepairInstanceResult,
};
use uuid::Uuid;

use crate::launcher::{self, JavaLaunchSpec, LaunchBackend, StartedJvm};

const JAVA_RUNTIME_MANIFEST_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

fn safe_component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && s != "."
        && s != ".."
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) async fn ensure_no_symlink(root: &Path, target: &Path) -> Result<(), String> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| "Path is outside the managed data directory".to_string())?;
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|e| e.to_string())?;
    let mut current = tokio::fs::canonicalize(root)
        .await
        .map_err(|e| e.to_string())?;
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("Unsafe managed path component".into());
        }
        current.push(component.as_os_str());
        match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err("Symbolic links are not allowed inside managed paths".into());
                }
                if index + 1 < components.len() && !metadata.is_dir() {
                    return Err("A managed path parent is not a directory".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

pub(super) fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("UMC/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "https" {
                attempt.follow()
            } else {
                attempt.error("UMC requires HTTPS for launcher downloads")
            }
        }))
        .build()
        .map_err(|e| e.to_string())
}

async fn request_bytes(
    client: &reqwest::Client,
    url: &str,
    task: Option<&super::tasks::TaskContext>,
) -> Result<Vec<u8>, String> {
    validate_download_url(url)?;
    let send = client.get(url).send();
    let response = match task {
        Some(context) => tokio::select! {
            _ = context.wait_cancelled() => return Err("Task cancelled".into()),
            result = tokio::time::timeout(Duration::from_secs(45), send) => result.map_err(|_| "Metadata request timed out".to_string())?.map_err(|e| e.to_string())?,
        },
        None => tokio::time::timeout(Duration::from_secs(45), send).await.map_err(|_| "Metadata request timed out".to_string())?.map_err(|e| e.to_string())?,
    }
    .error_for_status()
    .map_err(|e| e.to_string())?;
    let bytes = match task {
        Some(context) => tokio::select! {
            _ = context.wait_cancelled() => return Err("Task cancelled".into()),
            result = tokio::time::timeout(Duration::from_secs(45), response.bytes()) => result.map_err(|_| "Metadata response timed out".to_string())?.map_err(|e| e.to_string())?,
        },
        None => tokio::time::timeout(Duration::from_secs(45), response.bytes())
            .await
            .map_err(|_| "Metadata response timed out".to_string())?
            .map_err(|e| e.to_string())?,
    };
    Ok(bytes.to_vec())
}

async fn download(
    client: &reqwest::Client,
    root: &Path,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    task: Option<&super::tasks::TaskContext>,
) -> Result<(), String> {
    validate_download_url(url)?;
    ensure_no_symlink(root, dest).await?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(hash) = expected_sha1 {
        if verify_file(dest, hash, expected_size).await {
            return Ok(());
        }
    }
    let mut last_error = String::new();
    for attempt in 1..=3 {
        if task.is_some_and(|context| context.cancelled()) {
            return Err("Task cancelled".into());
        }
        match download_once(
            client,
            url,
            dest,
            expected_sha1,
            expected_size,
            task,
            attempt,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) if error == "Task cancelled" => return Err(error),
            Err(error) => {
                last_error = error;
                if attempt < 3 {
                    if let Some(context) = task {
                        context.emit("download.retry", serde_json::json!({"url":url,"attempt":attempt,"next_attempt":attempt + 1,"error":last_error}));
                        tokio::select! {
                            _ = context.wait_cancelled() => return Err("Task cancelled".into()),
                            _ = tokio::time::sleep(Duration::from_millis(300 * attempt as u64)) => {}
                        }
                    } else {
                        tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
                    }
                }
            }
        }
    }
    Err(format!("Download failed after 3 attempts: {last_error}"))
}

async fn download_once(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    task: Option<&super::tasks::TaskContext>,
    attempt: usize,
) -> Result<(), String> {
    use super::tasks::TaskContext;
    let _permit = match task {
        Some(context) => Some(context.download_permit().await?),
        None => None,
    };
    let send = client.get(url).send();
    let response = match task {
        Some(context) => tokio::select! {
            _ = context.wait_cancelled() => return Err("Task cancelled".into()),
            result = tokio::time::timeout(Duration::from_secs(45), send) => result.map_err(|_| "Download request timed out".to_string())?.map_err(|e| e.to_string())?,
        },
        None => tokio::time::timeout(Duration::from_secs(45), send).await.map_err(|_| "Download request timed out".to_string())?.map_err(|e| e.to_string())?,
    }.error_for_status().map_err(|e| e.to_string())?;
    let tmp = dest.with_extension(format!("part-{}", Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .await
        .map_err(|e| e.to_string())?;
    let transfer = async {
        let mut response = response;
        let mut hasher = Sha1::new();
        let mut downloaded = 0u64;
        let mut last_report = tokio::time::Instant::now();
        loop {
            let next = match task {
                Some(context) => tokio::select! {
                    _ = context.wait_cancelled() => return Err("Task cancelled".to_string()),
                    result = tokio::time::timeout(Duration::from_secs(45), response.chunk()) => result.map_err(|_| "Download stalled for 45 seconds".to_string())?.map_err(|e| e.to_string())?,
                },
                None => tokio::time::timeout(Duration::from_secs(45), response.chunk()).await.map_err(|_| "Download stalled for 45 seconds".to_string())?.map_err(|e| e.to_string())?,
            };
            let Some(chunk) = next else { break; };
            downloaded += chunk.len() as u64;
            if expected_size.is_some_and(|size| downloaded > size) { return Err(format!("Downloaded size exceeds metadata for {url}")); }
            hasher.update(&chunk);
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            if let Some(context) = task {
                if downloaded == chunk.len() as u64
                    || last_report.elapsed() >= Duration::from_millis(200)
                {
                    let percentage = expected_size.map(|total| if total == 0 { 100.0 } else { downloaded as f64 * 100.0 / total as f64 });
                    context.emit("download.progress", serde_json::json!({"url":url,"downloaded_bytes":downloaded,"total_bytes":expected_size,"percentage":percentage,"attempt":attempt}));
                    last_report = tokio::time::Instant::now();
                }
            }
        }
        file.flush().await.map_err(|e| e.to_string())?;
        if expected_size.is_some_and(|size| size != downloaded) { return Err(format!("Downloaded size mismatch for {url}")); }
        if expected_sha1.is_some_and(|expected| !hex(&hasher.finalize()).eq_ignore_ascii_case(expected)) { return Err(format!("SHA-1 mismatch for {url}")); }
        Ok::<u64, String>(downloaded)
    }.await;
    drop(file);
    let downloaded = match transfer {
        Ok(downloaded) => downloaded,
        Err(error) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(error);
        }
    };
    if task.is_some_and(TaskContext::cancelled) {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err("Task cancelled".into());
    }
    if cfg!(target_os = "windows") && tokio::fs::try_exists(dest).await.unwrap_or(false) {
        tokio::fs::remove_file(dest)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::rename(&tmp, dest)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(context) = task {
        context.emit("download.completed", serde_json::json!({"url":url,"downloaded_bytes":downloaded,"total_bytes":expected_size,"attempt":attempt}));
    }
    Ok(())
}

fn validate_download_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err("UMC only downloads launcher data over HTTPS".into())
    }
}

fn collect_asset_objects(index: &Value) -> Result<std::collections::HashMap<String, u64>, String> {
    let objects = index["objects"]
        .as_object()
        .ok_or("Asset index missing objects")?;
    let mut unique = std::collections::HashMap::new();
    for object in objects.values() {
        let hash = object["hash"].as_str().ok_or("Asset object missing hash")?;
        if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid asset SHA-1".into());
        }
        let size = object["size"].as_u64().ok_or("Asset object missing size")?;
        if unique.get(hash).is_some_and(|old| *old != size) {
            return Err("Conflicting sizes for identical asset hash".into());
        }
        unique.insert(hash.to_owned(), size);
    }
    Ok(unique)
}

async fn file_sha1(path: &Path) -> Result<(String, u64), String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((hex(&hasher.finalize()), size))
}

async fn verify_file(path: &Path, expected_sha1: &str, expected_size: Option<u64>) -> bool {
    match file_sha1(path).await {
        Ok((hash, size)) => {
            hash.eq_ignore_ascii_case(expected_sha1)
                && expected_size.is_none_or(|expected| expected == size)
        }
        Err(_) => false,
    }
}

pub async fn create_instance(
    root: &Path,
    id: &str,
    name: String,
    minecraft_version: String,
    memory: Option<MemoryConfig>,
) -> Result<InstanceConfig, String> {
    if !safe_component(id) || !safe_component(&minecraft_version) {
        return Err("Invalid instance or Minecraft version identifier".into());
    }
    if name.trim().is_empty() || name.len() > 128 {
        return Err("Instance name must contain 1 to 128 characters".into());
    }
    if !version_exists(root, &minecraft_version).await? {
        return Err(format!("Unknown Minecraft version: {minecraft_version}"));
    }
    let memory = memory.unwrap_or(MemoryConfig {
        minimum_mib: 1024,
        maximum_mib: 4096,
    });
    if memory.minimum_mib == 0
        || memory.maximum_mib < memory.minimum_mib
        || memory.maximum_mib > 1_048_576
    {
        return Err("Invalid memory range in MiB".into());
    }
    let cfg = InstanceConfig {
        schema_version: INSTANCE_CONFIG_SCHEMA_VERSION,
        id: id.into(),
        name,
        minecraft_version,
        loader: None,
        java_runtime: "auto".into(),
        memory,
        game_directory: format!("instances/{id}/game"),
    };
    let dir = root.join("instances").join(id);
    ensure_no_symlink(root, &dir.join("game")).await?;
    tokio::fs::create_dir_all(dir.join("game"))
        .await
        .map_err(|e| e.to_string())?;
    write_config(root, &dir.join("instance.json"), &cfg).await?;
    Ok(cfg)
}

async fn atomic_write(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    ensure_no_symlink(root, path).await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    ensure_no_symlink(root, &temp).await?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .await
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).await.map_err(|e| e.to_string())?;
    file.flush().await.map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    if cfg!(target_os = "windows") && tokio::fs::try_exists(path).await.unwrap_or(false) {
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::rename(&temp, path)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        tokio::fs::File::open(parent)
            .await
            .map_err(|e| e.to_string())?
            .sync_all()
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn write_config(root: &Path, path: &Path, cfg: &InstanceConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    let backup = path.with_extension("json.bak");
    if let Ok(existing) = tokio::fs::read(path).await {
        if parse_config(&existing, &cfg.id).is_ok() {
            atomic_write(root, &backup, &existing).await?;
        }
    }
    atomic_write(root, path, &bytes).await
}

pub async fn list_instances(root: &Path) -> Result<Vec<InstanceConfig>, String> {
    let dir = root.join("instances");
    ensure_no_symlink(root, &dir).await?;
    let mut out = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.to_string()),
    };
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let file_type = entry.file_type().await.map_err(|e| e.to_string())?;
        if file_type.is_symlink() {
            return Err(format!(
                "Instance directory {} is a symbolic link",
                entry.file_name().to_string_lossy()
            ));
        }
        if !file_type.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path().join("instance.json");
        ensure_no_symlink(root, &path)
            .await
            .map_err(|error| format!("Instance {id} needs repair: {error}"))?;
        match read_config(root, &id).await {
            Ok(cfg) => out.push(cfg),
            Err(error) if error.contains("not found") => continue,
            Err(error) => return Err(format!("Instance {id} needs repair: {error}")),
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub async fn get_instance(root: &Path, id: &str) -> Result<InstanceConfig, String> {
    read_config(root, id).await
}

async fn read_config(root: &Path, id: &str) -> Result<InstanceConfig, String> {
    if !safe_component(id) {
        return Err("Invalid instance identifier".into());
    }
    let config_path = root.join("instances").join(id).join("instance.json");
    ensure_no_symlink(root, &config_path).await?;
    let primary = tokio::fs::read(&config_path).await;
    if let Ok(bytes) = &primary {
        match parse_config(bytes, id) {
            Ok((cfg, migrated)) => {
                if migrated {
                    write_config(root, &config_path, &cfg).await?;
                }
                return Ok(cfg);
            }
            Err(error) if error.starts_with("Instance schema ") => return Err(error),
            Err(_) => {}
        }
    }
    let backup_path = config_path.with_extension("json.bak");
    ensure_no_symlink(root, &backup_path).await?;
    if let Ok(bytes) = tokio::fs::read(&backup_path).await {
        match parse_config(&bytes, id) {
            Ok((cfg, _)) => {
                if primary.is_ok() {
                    quarantine(&config_path).await?;
                }
                write_config(root, &config_path, &cfg).await?;
                return Ok(cfg);
            }
            Err(error) if error.starts_with("Instance schema ") => return Err(error),
            Err(_) => {}
        }
    }
    match primary {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("Instance not found: {id}"))
        }
        Err(error) => Err(format!(
            "Could not read instance configuration for {id}: {error}"
        )),
        Ok(_) => Err(format!(
            "Instance configuration for {id} is corrupt and no valid backup exists; use instance.repair if an installation record is available"
        )),
    }
}

pub async fn repair_instance(root: &Path, id: &str) -> Result<RepairInstanceResult, String> {
    if !safe_component(id) {
        return Err("Invalid instance identifier".into());
    }
    let dir = root.join("instances").join(id);
    let config_path = dir.join("instance.json");
    let backup_path = config_path.with_extension("json.bak");
    ensure_no_symlink(root, &config_path).await?;
    ensure_no_symlink(root, &backup_path).await?;
    if let Ok(bytes) = tokio::fs::read(&config_path).await {
        match parse_config(&bytes, id) {
            Ok(_) => {
                let config = read_config(root, id).await?;
                return Ok(RepairInstanceResult {
                    config,
                    recovered_from: "primary_configuration".into(),
                });
            }
            Err(error) if error.starts_with("Instance schema ") => return Err(error),
            Err(_) => {}
        }
    }
    if let Ok(bytes) = tokio::fs::read(&backup_path).await {
        match parse_config(&bytes, id) {
            Ok(_) => {
                let config = read_config(root, id).await?;
                return Ok(RepairInstanceResult {
                    config,
                    recovered_from: "backup".into(),
                });
            }
            Err(error) if error.starts_with("Instance schema ") => return Err(error),
            Err(_) => {}
        }
    }
    let state_path = dir.join("install.json");
    ensure_no_symlink(root, &state_path).await?;
    let state: Value =
        serde_json::from_slice(&tokio::fs::read(&state_path).await.map_err(|_| {
            "No valid backup or installation record is available to repair this instance"
                .to_string()
        })?)
        .map_err(|e| format!("Installation record is corrupt: {e}"))?;
    let version = state["version"]
        .as_str()
        .ok_or("Installation record has no Minecraft version")?;
    if !safe_component(version)
        || !state["classpath"]
            .as_array()
            .is_some_and(|paths| paths.iter().all(Value::is_string))
    {
        return Err(
            "Installation record does not contain enough valid data to repair this instance".into(),
        );
    }
    let metadata_path = root
        .join("versions")
        .join(version)
        .join(format!("{version}.json"));
    ensure_no_symlink(root, &metadata_path).await?;
    let metadata: Value = serde_json::from_slice(
        &tokio::fs::read(&metadata_path)
            .await
            .map_err(|_| "Version metadata is missing; reinstall before repairing".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if metadata["id"].as_str() != Some(version)
        || state["main_class"].as_str() != metadata["mainClass"].as_str()
    {
        return Err("Installation record does not match the downloaded version metadata".into());
    }
    for item in state["classpath"].as_array().into_iter().flatten() {
        let path = Path::new(item.as_str().ok_or("Invalid classpath item")?);
        if !path.is_absolute() || path.strip_prefix(root).is_err() {
            return Err("Installation classpath escapes the managed data directory".into());
        }
    }
    quarantine(&config_path).await?;
    quarantine(&backup_path).await?;
    let config = InstanceConfig {
        schema_version: INSTANCE_CONFIG_SCHEMA_VERSION,
        id: id.into(),
        name: id.into(),
        minecraft_version: version.into(),
        loader: None,
        java_runtime: "auto".into(),
        memory: MemoryConfig {
            minimum_mib: 1024,
            maximum_mib: 4096,
        },
        game_directory: format!("instances/{id}/game"),
    };
    write_config(root, &config_path, &config).await?;
    Ok(RepairInstanceResult {
        config,
        recovered_from: "installation_record".into(),
    })
}

fn parse_config(bytes: &[u8], expected_id: &str) -> Result<(InstanceConfig, bool), String> {
    let mut cfg: InstanceConfig =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid instance JSON: {e}"))?;
    if cfg.schema_version > INSTANCE_CONFIG_SCHEMA_VERSION {
        return Err(format!(
            "Instance schema {} is newer than this Core supports",
            cfg.schema_version
        ));
    }
    let migrated = cfg.schema_version < INSTANCE_CONFIG_SCHEMA_VERSION;
    if cfg.schema_version == 0 {
        cfg.schema_version = 1;
    }
    if cfg.schema_version != INSTANCE_CONFIG_SCHEMA_VERSION {
        return Err("Unsupported instance schema version".into());
    }
    if cfg.id != expected_id || !safe_component(&cfg.id) || !safe_component(&cfg.minecraft_version)
    {
        return Err("Instance identity or Minecraft version is invalid".into());
    }
    if cfg.name.trim().is_empty() || cfg.name.len() > 128 {
        return Err("Instance name is invalid".into());
    }
    if cfg.memory.minimum_mib == 0
        || cfg.memory.maximum_mib < cfg.memory.minimum_mib
        || cfg.memory.maximum_mib > 1_048_576
    {
        return Err("Instance memory range is invalid".into());
    }
    if cfg.game_directory != format!("instances/{expected_id}/game") {
        return Err("Instance game directory is outside its managed path".into());
    }
    if cfg.java_runtime.trim().is_empty() || cfg.java_runtime.len() > 1024 {
        return Err("Instance Java runtime setting is invalid".into());
    }
    Ok((cfg, migrated))
}

async fn quarantine(path: &Path) -> Result<(), String> {
    if tokio::fs::try_exists(path).await.unwrap_or(false) {
        let name = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("Invalid config filename")?;
        let quarantined = path.with_file_name(format!("{name}.corrupt-{}", Uuid::new_v4()));
        tokio::fs::rename(path, quarantined)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn remove_instance(root: &Path, id: &str) -> Result<(), String> {
    let _ = read_config(root, id).await?;
    ensure_no_symlink(root, &root.join("processes")).await?;
    match tokio::fs::read_dir(root.join("processes")).await {
        Ok(mut entries) => {
            while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
                if entry.path().extension().and_then(|v| v.to_str()) != Some("json") {
                    continue;
                }
                ensure_no_symlink(root, &entry.path()).await.map_err(|e| {
                    format!("Cannot verify game process state; refusing removal: {e}")
                })?;
                let raw = tokio::fs::read(entry.path()).await.map_err(|e| {
                    format!("Cannot verify game process state; refusing removal: {e}")
                })?;
                let process = serde_json::from_slice::<GameProcess>(&raw)
                    .map_err(|e| format!("Corrupt game process state; refusing removal: {e}"))?;
                if process.instance_id == id
                    && matches!(
                        process.status,
                        ProcessStatus::Running | ProcessStatus::Starting
                    )
                {
                    return Err("Stop this instance's game process before removing it".into());
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Cannot verify game process state; refusing removal: {error}"
            ));
        }
    }
    let dir = root.join("instances").join(id);
    ensure_no_symlink(root, &dir).await?;
    tokio::fs::remove_dir_all(dir)
        .await
        .map_err(|e| e.to_string())
}

pub async fn process_status(root: &Path, process_id: &str) -> Result<GameProcess, String> {
    if !safe_component(process_id) {
        return Err("Invalid process identifier".into());
    }
    let path = root.join("processes").join(format!("{process_id}.json"));
    ensure_no_symlink(root, &path).await?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| "Game process not found".to_string())?;
    let mut process: GameProcess = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if matches!(
        process.status,
        ProcessStatus::Running | ProcessStatus::Starting
    ) {
        if process.kind == ProcessKind::Embedded {
            // An embedded JVM has no pid: its state comes from the C API.
            match launcher::embedded_status() {
                Some(launcher::MAIN_RUNNING) => {}
                Some(launcher::MAIN_FINISHED) => {
                    process.status = ProcessStatus::Exited;
                    process.exit_code = Some(0);
                    write_process_record(&path, &process)?;
                }
                Some(launcher::MAIN_FAILED) => {
                    process.status = ProcessStatus::Failed;
                    process.exit_code = Some(-1);
                    write_process_record(&path, &process)?;
                }
                _ => {
                    process.status = ProcessStatus::Exited;
                    process.exit_code = None;
                    write_process_record(&path, &process)?;
                }
            }
        } else if !process_matches(process.pid, &process.process_id) {
            process.status = ProcessStatus::Exited;
            process.exit_code = None;
            write_process_record(&path, &process)?;
        }
    }
    Ok(process)
}

pub async fn list_processes(root: &Path) -> Result<Vec<GameProcess>, String> {
    let dir = root.join("processes");
    ensure_no_symlink(root, &dir).await?;
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut processes = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let id = entry
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid process state filename")?
            .to_owned();
        processes.push(process_status(root, &id).await?);
    }
    processes.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    Ok(processes)
}

pub async fn recover_processes(root: &Path) -> Result<Vec<GameProcess>, String> {
    list_processes(root).await
}

pub async fn stop_process(root: &Path, process_id: &str) -> Result<(), String> {
    let process = process_status(root, process_id).await?;
    if !matches!(
        process.status,
        ProcessStatus::Running | ProcessStatus::Starting
    ) {
        return Err("Game process is not running".into());
    }
    if process.kind == ProcessKind::Embedded {
        // The embedded JVM shares the launcher process; signalling it would
        // kill the app. A dedicated shutdown hook is not available yet.
        return Err("Stopping an embedded JVM is not supported yet".into());
    }
    if !process_matches(process.pid, process_id) {
        return Err(
            "Game process identity could not be verified; refusing to signal the PID".into(),
        );
    }
    terminate_process(process.pid)
}

fn write_process_record(path: &Path, process: &GameProcess) -> Result<(), String> {
    let temp = path.with_extension(format!("json.{}.tmp", Uuid::new_v4()));
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(process).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    set_private_file(&temp).map_err(|e| format!("Could not secure process state file: {e}"))?;
    if cfg!(windows) && path.exists() {
        std::fs::remove_file(path)
            .map_err(|e| format!("Could not replace process state file: {e}"))?;
    }
    std::fs::rename(&temp, path).map_err(|e| format!("Could not commit process state file: {e}"))
}

fn process_matches(pid: u32, process_id: &str) -> bool {
    let marker = format!("-Dumc.process_id={process_id}");
    #[cfg(target_os = "linux")]
    {
        return std::fs::read(format!("/proc/{pid}/cmdline"))
            .is_ok_and(|cmd| cmd.split(|b| *b == 0).any(|arg| arg == marker.as_bytes()));
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        return std::process::Command::new("ps")
            .args(["-o", "command=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).contains(&marker)
            });
    }
    #[cfg(windows)]
    {
        let wmic = std::process::Command::new("wmic.exe")
            .args([
                "process",
                "where",
                &format!("ProcessId={pid}"),
                "get",
                "CommandLine",
                "/VALUE",
            ])
            .output()
            .is_ok_and(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).contains(&marker)
            });
        if wmic {
            return true;
        }
        let query = format!(
            "Get-CimInstance Win32_Process -Filter 'ProcessId = {pid}' | Select-Object -ExpandProperty CommandLine"
        );
        return std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &query])
            .output()
            .is_ok_and(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).contains(&marker)
            });
    }
    #[allow(unreachable_code)]
    false
}

fn terminate_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if result == 0 {
            return Ok(());
        }
        return Err(std::io::Error::last_os_error().to_string());
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("taskkill.exe")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        Err("Process termination is unsupported on this platform".into())
    }
}

#[cfg(unix)]
pub(super) fn set_private_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
pub(super) fn set_private_file(path: &Path) -> Result<(), String> {
    set_windows_acl(path, "D:P(A;;FA;;;OW)")
}

#[cfg(not(any(unix, windows)))]
pub(super) fn set_private_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
pub(super) async fn set_private_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
pub(super) async fn set_private_dir(path: &Path) -> Result<(), String> {
    set_windows_acl(path, "D:P(A;OICI;FA;;;OW)")
}

#[cfg(not(any(unix, windows)))]
pub(super) async fn set_private_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_private_dir_blocking(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
pub(super) fn set_private_dir_blocking(path: &Path) -> Result<(), String> {
    set_windows_acl(path, "D:P(A;OICI;FA;;;OW)")
}

#[cfg(not(any(unix, windows)))]
pub(super) fn set_private_dir_blocking(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn set_windows_acl(path: &Path, sddl_text: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::{ffi::c_void, ptr};
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
        fn SetFileSecurityW(path: *const u16, information: u32, descriptor: *mut c_void) -> i32;
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    let sddl: Vec<u16> = sddl_text.encode_utf16().chain(std::iter::once(0)).collect();
    let mut descriptor = ptr::null_mut();
    let mut size = 0;
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            &mut size,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe { SetFileSecurityW(path.as_ptr(), 0x00000004, descriptor) };
    unsafe { LocalFree(descriptor) };
    let _ = size;
    if result == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

async fn version_exists(root: &Path, id: &str) -> Result<bool, String> {
    let entries = fetch_manifest(root).await?;
    Ok(entries
        .iter()
        .any(|v| v.get("id").and_then(Value::as_str) == Some(id)))
}

async fn fetch_manifest(root: &Path) -> Result<Vec<Value>, String> {
    fetch_manifest_task(root, None).await
}

async fn fetch_manifest_task(
    root: &Path,
    task: Option<&super::tasks::TaskContext>,
) -> Result<Vec<Value>, String> {
    let client = http_client()?;
    let cache = root.join("cache/version_manifest_v2.json");
    ensure_no_symlink(root, &cache).await?;
    let use_cache = tokio::fs::metadata(&cache)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 3600);
    if use_cache {
        if let Ok(raw) = tokio::fs::read(&cache).await {
            if let Ok(doc) = serde_json::from_slice::<Value>(&raw) {
                if let Some(v) = doc["versions"].as_array() {
                    return Ok(v.clone());
                }
            }
        }
    }
    let bytes = request_bytes(
        &client,
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        task,
    )
    .await?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let versions = doc["versions"]
        .as_array()
        .ok_or("Manifest missing versions")?
        .clone();
    tokio::fs::create_dir_all(cache.parent().unwrap())
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::write(cache, bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(versions)
}

pub async fn install_vanilla(
    root: &Path,
    id: &str,
    task: &super::tasks::TaskContext,
) -> Result<(), String> {
    task.emit(
        "install.progress",
        serde_json::json!({"stage":"instance_validation"}),
    );
    let cfg = read_config(root, id).await?;
    if cfg.loader.is_some() {
        return Err("This installer currently supports vanilla instances only".into());
    }
    task.emit(
        "install.progress",
        serde_json::json!({"stage":"version_manifest"}),
    );
    let versions = fetch_manifest_task(root, Some(task)).await?;
    let entry = versions
        .iter()
        .find(|v| v["id"].as_str() == Some(&cfg.minecraft_version))
        .ok_or("Version missing from official manifest")?;
    let client = http_client()?;
    let version_dir = root.join("versions").join(&cfg.minecraft_version);
    let version_json = version_dir.join(format!("{}.json", cfg.minecraft_version));
    let version_hash = entry["sha1"]
        .as_str()
        .ok_or("Manifest version entry has no SHA-1")?;
    download(
        &client,
        root,
        entry["url"]
            .as_str()
            .ok_or("Manifest version entry has no URL")?,
        &version_json,
        Some(version_hash),
        entry["size"].as_u64(),
        Some(task),
    )
    .await?;
    let metadata: Value = serde_json::from_slice(
        &tokio::fs::read(&version_json)
            .await
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let client_artifact = &metadata["downloads"]["client"];
    download(
        &client,
        root,
        client_artifact["url"]
            .as_str()
            .ok_or("Version metadata has no client download")?,
        &version_dir.join(format!("{}.jar", cfg.minecraft_version)),
        client_artifact["sha1"].as_str(),
        client_artifact["size"].as_u64(),
        Some(task),
    )
    .await?;

    let mut classpath = Vec::new();
    for lib in metadata["libraries"].as_array().into_iter().flatten() {
        if !rules_allow(lib.get("rules"), &std::collections::HashMap::new()) {
            continue;
        }
        let artifact = &lib["downloads"]["artifact"];
        if artifact.is_object() {
            let rel = artifact["path"]
                .as_str()
                .ok_or("Library artifact missing path")?;
            let path = safe_relative(root, rel)?;
            download(
                &client,
                root,
                artifact["url"]
                    .as_str()
                    .ok_or("Library artifact missing URL")?,
                &path,
                artifact["sha1"].as_str(),
                artifact["size"].as_u64(),
                Some(task),
            )
            .await?;
            classpath.push(path.to_string_lossy().into_owned());
        }
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        };
        if let Some(native_key) = lib
            .get("natives")
            .and_then(|n| n.get(platform))
            .and_then(Value::as_str)
        {
            let arch = if cfg!(target_pointer_width = "64") {
                "64"
            } else {
                "32"
            };
            let classifier = native_key.replace("${arch}", arch);
            let native = &lib["downloads"]["classifiers"][classifier];
            if native.is_object() {
                let rel = native["path"]
                    .as_str()
                    .ok_or("Native library missing path")?;
                let jar = safe_relative(root, rel)?;
                download(
                    &client,
                    root,
                    native["url"].as_str().ok_or("Native library missing URL")?,
                    &jar,
                    native["sha1"].as_str(),
                    native["size"].as_u64(),
                    Some(task),
                )
                .await?;
                let destination = root.join("natives").join(&cfg.minecraft_version);
                ensure_no_symlink(root, &destination).await?;
                tokio::fs::create_dir_all(&destination)
                    .await
                    .map_err(|e| e.to_string())?;
                let exclude = lib["extract"]["exclude"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let jar_copy = jar.clone();
                let dest_copy = destination.clone();
                tokio::task::spawn_blocking(move || -> Result<(), String> {
                    let file = File::open(jar_copy).map_err(|e| e.to_string())?;
                    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                    for i in 0..archive.len() {
                        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                        let Some(rel_path) = entry.enclosed_name() else {
                            continue;
                        };
                        let rel = rel_path.to_string_lossy();
                        if rel.starts_with("META-INF/")
                            || exclude
                                .iter()
                                .any(|v| v.as_str().is_some_and(|x| rel.starts_with(x)))
                        {
                            continue;
                        }
                        let out = dest_copy.join(rel_path);
                        if entry.is_dir() {
                            std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
                        } else {
                            if let Some(parent) = out.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                            }
                            let mut file = File::create(out).map_err(|e| e.to_string())?;
                            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())??;
            }
        }
    }
    classpath.push(
        version_dir
            .join(format!("{}.jar", cfg.minecraft_version))
            .to_string_lossy()
            .into_owned(),
    );

    let mut assets_index = String::new();
    let mut virtual_assets = String::new();
    if metadata["assetIndex"].is_object() {
        let ai = &metadata["assetIndex"];
        assets_index = ai["id"].as_str().ok_or("Asset index missing id")?.into();
        let index_path = root
            .join("assets/indexes")
            .join(format!("{assets_index}.json"));
        download(
            &client,
            root,
            ai["url"].as_str().ok_or("Asset index missing URL")?,
            &index_path,
            ai["sha1"].as_str(),
            ai["size"].as_u64(),
            Some(task),
        )
        .await?;
        let index: Value = serde_json::from_slice(
            &tokio::fs::read(&index_path)
                .await
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let virtual_flag = index["virtual"].as_bool().unwrap_or(false);
        let map_resources = index["map_to_resources"].as_bool().unwrap_or(false);
        if virtual_flag {
            virtual_assets = if map_resources {
                "legacy".into()
            } else {
                assets_index.clone()
            };
        }
        let objects = index["objects"]
            .as_object()
            .ok_or("Asset index missing objects")?;
        let unique_objects = collect_asset_objects(&index)?;
        stream::iter(unique_objects.into_iter().map(|(hash, size)| {
            let client = &client;
            let task = task.clone();
            async move {
                let dest = root.join("assets/objects").join(&hash[..2]).join(hash);
                let rel_hash = dest
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or("Invalid asset hash path")?;
                let url = format!(
                    "https://resources.download.minecraft.net/{}/{}",
                    &rel_hash[..2],
                    rel_hash
                );
                download(
                    client,
                    root,
                    &url,
                    &dest,
                    Some(rel_hash),
                    Some(size),
                    Some(&task),
                )
                .await?;
                Ok::<(), String>(())
            }
        }))
        .buffer_unordered(32)
        .try_collect::<Vec<_>>()
        .await?;

        for (asset_name, object) in objects {
            let hash = object["hash"].as_str().ok_or("Asset object missing hash")?;
            let dest = root.join("assets/objects").join(&hash[..2]).join(hash);
            if virtual_flag {
                let rel = format!("assets/virtual/{virtual_assets}/{asset_name}");
                let alias = safe_relative(root, &rel)?;
                ensure_no_symlink(root, &alias).await?;
                if let Some(parent) = alias.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                tokio::fs::copy(&dest, &alias)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            if map_resources {
                let rel = format!("instances/{id}/game/resources/{asset_name}");
                let alias = safe_relative(root, &rel)?;
                ensure_no_symlink(root, &alias).await?;
                if let Some(parent) = alias.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                tokio::fs::copy(&dest, &alias)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    let mut log_config = String::new();
    if metadata["logging"]["client"]["file"].is_object() {
        let file = &metadata["logging"]["client"]["file"];
        let path = safe_relative(root, file["id"].as_str().ok_or("Log config has no id")?)?;
        download(
            &client,
            root,
            file["url"].as_str().ok_or("Log config has no URL")?,
            &path,
            file["sha1"].as_str(),
            file["size"].as_u64(),
            Some(task),
        )
        .await?;
        log_config = path.to_string_lossy().into_owned();
    }
    // Persist resolved launch context beside the instance config.
    let state = serde_json::json!({"version":cfg.minecraft_version,"main_class":metadata["mainClass"],"assets_index":assets_index,"virtual_assets":virtual_assets,"classpath":classpath,"log_config":log_config});
    let install_path = root.join("instances").join(id).join("install.json");
    ensure_no_symlink(root, &install_path).await?;
    tokio::fs::write(
        install_path,
        serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn safe_relative(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Unsafe path in official metadata".into());
    }
    Ok(root.join(path))
}

fn rules_allow(rules: Option<&Value>, features: &std::collections::HashMap<&str, bool>) -> bool {
    let Some(rules) = rules.and_then(Value::as_array) else {
        return true;
    };
    let mut allowed = false;
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };
    for rule in rules {
        let platform = rule
            .get("os")
            .and_then(|o| o.get("name"))
            .and_then(Value::as_str);
        let feature_match =
            rule.get("features")
                .and_then(Value::as_object)
                .is_none_or(|conditions| {
                    conditions.iter().all(|(name, expected)| {
                        features
                            .get(name.as_str())
                            .is_some_and(|actual| expected.as_bool() == Some(*actual))
                    })
                });
        if (platform.is_none() || platform == Some(os)) && feature_match {
            allowed = rule["action"].as_str() == Some("allow");
        }
    }
    allowed
}

pub(super) fn timestamp_rfc3339() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }).div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3600,
        day_seconds % 3600 / 60,
        day_seconds % 60
    )
}

pub fn detect_java(explicit: Option<&str>, minimum: u32) -> Result<JavaRuntimeInfo, String> {
    let candidates = if let Some(path) = explicit {
        vec![PathBuf::from(path)]
    } else {
        let bin = if cfg!(target_os = "windows") {
            "java.exe"
        } else {
            "java"
        };
        let mut v = Vec::new();
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            v.push(PathBuf::from(home).join("bin").join(bin));
        }
        if let Some(paths) = std::env::var_os("PATH") {
            v.extend(std::env::split_paths(&paths).map(|p| p.join(bin)));
        }
        v
    };
    let mut incompatible = None;
    for candidate in candidates {
        let out = Command::new(&candidate)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        let Ok(out) = out else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stderr);
        let version = text.split('"').nth(1).unwrap_or("");
        let first = version
            .split('.')
            .next()
            .unwrap_or("0")
            .parse::<u32>()
            .unwrap_or(0);
        let major = if first == 1 {
            version
                .split('.')
                .nth(1)
                .unwrap_or("0")
                .parse()
                .unwrap_or(0)
        } else {
            first
        };
        let info = JavaRuntimeInfo {
            path: candidate.to_string_lossy().into_owned(),
            major_version: major,
            vendor_version: version.into(),
            compatible: major >= minimum,
            required_major: minimum,
            runtime_id: None,
        };
        if info.compatible {
            return Ok(info);
        }
        incompatible.get_or_insert(info);
    }
    if let Some(info) = incompatible {
        return Ok(info);
    }
    Err(format!(
        "No Java runtime found (requires Java {minimum} or newer)"
    ))
}

fn java_runtime_platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("linux"),
        ("linux", "x86") => Ok("linux-i386"),
        ("macos", "x86_64") => Ok("mac-os"),
        ("macos", "aarch64") => Ok("mac-os-arm64"),
        ("windows", "x86_64") => Ok("windows-x64"),
        ("windows", "x86") => Ok("windows-x86"),
        ("windows", "aarch64") => Ok("windows-arm64"),
        _ => Err(format!(
            "No Mojang Java runtime is published for {}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
    }
}

fn safe_runtime_relative_path(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if raw.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!("Unsafe path in Java runtime manifest: {raw}"));
    }
    Ok(path.to_path_buf())
}

pub async fn install_java_runtime(
    root: &Path,
    minecraft_version: &str,
    task: &super::tasks::TaskContext,
) -> Result<JavaRuntimeInstallResult, String> {
    task.emit(
        "install.progress",
        serde_json::json!({"stage":"java_runtime_metadata","minecraft_version":minecraft_version}),
    );
    if !safe_component(minecraft_version) {
        return Err("Invalid Minecraft version".into());
    }
    let metadata_path = root
        .join("versions")
        .join(minecraft_version)
        .join(format!("{minecraft_version}.json"));
    ensure_no_symlink(root, &metadata_path).await?;
    let metadata: Value = serde_json::from_slice(&tokio::fs::read(&metadata_path).await.map_err(|_| format!("Minecraft {minecraft_version} is not installed; install it before provisioning Java"))?).map_err(|e| format!("Invalid Minecraft version metadata: {e}"))?;
    let java_version = metadata.get("javaVersion");
    let major = java_version
        .and_then(|v| v["majorVersion"].as_u64())
        .unwrap_or(8) as u32;
    let component = java_version
        .and_then(|v| v["component"].as_str())
        .unwrap_or("jre-legacy");
    if !matches!(
        component,
        "java-runtime-alpha"
            | "java-runtime-beta"
            | "java-runtime-gamma"
            | "java-runtime-delta"
            | "jre-legacy"
    ) {
        return Err(format!(
            "Unsupported Mojang Java runtime component: {component}"
        ));
    }
    let client = http_client()?;
    let all_bytes = request_bytes(&client, JAVA_RUNTIME_MANIFEST_URL, Some(task)).await?;
    let all: Value = serde_json::from_slice(&all_bytes)
        .map_err(|e| format!("Could not read Mojang Java runtime catalog: {e}"))?;
    let platform = java_runtime_platform()?;
    let targets = all
        .get(platform)
        .and_then(|p| p.get(component))
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Mojang has no {component} runtime for {platform}"))?;
    let target = targets
        .first()
        .ok_or_else(|| format!("Mojang has no published versions for {component}/{platform}"))?;
    let manifest_url = target["manifest"]["url"]
        .as_str()
        .ok_or("Java runtime catalog entry has no manifest URL")?;
    validate_download_url(manifest_url)?;
    let manifest_bytes = request_bytes(&client, manifest_url, Some(task)).await?;
    let expected_manifest_sha1 = target["manifest"]["sha1"]
        .as_str()
        .ok_or("Java runtime catalog entry has no manifest SHA-1")?;
    if expected_manifest_sha1.len() != 40
        || !expected_manifest_sha1
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || !hex(&Sha1::digest(&manifest_bytes)).eq_ignore_ascii_case(expected_manifest_sha1)
    {
        return Err("Mojang Java runtime manifest SHA-1 mismatch".into());
    }
    let runtime_manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("Invalid Java runtime manifest: {e}"))?;
    let runtime_version = target["version"]["name"]
        .as_str()
        .ok_or("Java runtime catalog entry has no version")?;
    if !safe_component(runtime_version) {
        return Err("Unsafe Java runtime version from Mojang".into());
    }
    let runtime_id = format!("{component}@{runtime_version}");
    let runtime_root = root.join("runtimes").join(component).join(runtime_version);
    ensure_no_symlink(root, &runtime_root).await?;
    let files = runtime_manifest["files"]
        .as_object()
        .ok_or("Java runtime manifest has no files map")?;
    let mut links = Vec::new();
    for (name, entry) in files {
        let relative = safe_runtime_relative_path(name)?;
        let dest = runtime_root.join(&relative);
        match entry["type"].as_str() {
            Some("directory") => {
                ensure_no_symlink(root, &dest).await?;
                tokio::fs::create_dir_all(&dest)
                    .await
                    .map_err(|e| e.to_string())?
            }
            Some("file") => {
                let raw = &entry["downloads"]["raw"];
                let url = raw["url"]
                    .as_str()
                    .ok_or_else(|| format!("Java runtime file {name} has no raw URL"))?;
                let sha1 = raw["sha1"]
                    .as_str()
                    .ok_or_else(|| format!("Java runtime file {name} has no SHA-1"))?;
                if sha1.len() != 40 || !sha1.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(format!("Invalid SHA-1 for Java runtime file {name}"));
                }
                let size = raw["size"]
                    .as_u64()
                    .ok_or_else(|| format!("Java runtime file {name} has no size"))?;
                download(
                    &client,
                    root,
                    url,
                    &dest,
                    Some(sha1),
                    Some(size),
                    Some(task),
                )
                .await
                .map_err(|e| format!("Java runtime download failed for {name}: {e}"))?;
                if entry["executable"].as_bool().unwrap_or(false) {
                    set_executable(&dest).await?;
                }
            }
            Some("link") => links.push((
                relative,
                entry["target"]
                    .as_str()
                    .ok_or("Java runtime link has no target")?
                    .to_owned(),
            )),
            other => {
                return Err(format!(
                    "Unsupported Java runtime manifest entry type: {other:?}"
                ));
            }
        }
    }
    for (relative, target) in links {
        let target_relative = safe_runtime_relative_path(&target)?;
        let source = runtime_root.join(target_relative);
        let dest = runtime_root.join(relative);
        ensure_no_symlink(root, &source).await?;
        if tokio::fs::metadata(&source)
            .await
            .map_err(|e| format!("Java runtime link target is missing: {e}"))?
            .is_file()
        {
            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            tokio::fs::copy(source, dest)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            return Err("Unsupported directory link in Java runtime manifest".into());
        }
    }
    let executable = if cfg!(windows) {
        runtime_root.join("bin/java.exe")
    } else {
        runtime_root.join("bin/java")
    };
    let detected = detect_java(Some(executable.to_string_lossy().as_ref()), major)
        .map_err(|e| format!("Downloaded Java runtime failed verification: {e}"))?;
    let java = JavaRuntimeInfo {
        compatible: detected.major_version == major,
        required_major: major,
        runtime_id: Some(runtime_id.clone()),
        ..detected
    };
    if !java.compatible {
        return Err(format!(
            "Downloaded Java {} is incompatible with required Java {major}",
            java.major_version
        ));
    }
    let marker = runtime_root.join("umc-runtime.json");
    ensure_no_symlink(root, &marker).await?;
    tokio::fs::write(
        &marker,
        serde_json::to_vec(&java).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    task.emit(
        "install.progress",
        serde_json::json!({"stage":"java_runtime_verified","runtime_id":runtime_id}),
    );
    Ok(JavaRuntimeInstallResult {
        runtime_id,
        minecraft_version: minecraft_version.into(),
        java,
    })
}
#[cfg(unix)]
async fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(not(unix))]
async fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

pub async fn list_java_runtimes(root: &Path) -> Result<Vec<JavaRuntimeInfo>, String> {
    let base = root.join("runtimes");
    ensure_no_symlink(root, &base).await?;
    let mut result = Vec::new();
    let mut components = match tokio::fs::read_dir(&base).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(e) => return Err(e.to_string()),
    };
    while let Some(component) = components.next_entry().await.map_err(|e| e.to_string())? {
        ensure_no_symlink(root, &component.path()).await?;
        if !component
            .file_type()
            .await
            .map_err(|e| e.to_string())?
            .is_dir()
        {
            continue;
        }
        let mut versions = tokio::fs::read_dir(component.path())
            .await
            .map_err(|e| e.to_string())?;
        while let Some(version) = versions.next_entry().await.map_err(|e| e.to_string())? {
            ensure_no_symlink(root, &version.path()).await?;
            let marker = version.path().join("umc-runtime.json");
            ensure_no_symlink(root, &marker).await?;
            if !tokio::fs::try_exists(&marker).await.unwrap_or(false) {
                continue;
            }
            let java = if cfg!(windows) {
                version.path().join("bin/java.exe")
            } else {
                version.path().join("bin/java")
            };
            if let Ok(mut info) = detect_java(Some(java.to_string_lossy().as_ref()), 1) {
                info.runtime_id = Some(format!(
                    "{}@{}",
                    component.file_name().to_string_lossy(),
                    version.file_name().to_string_lossy()
                ));
                info.required_major = 0;
                result.push(info);
            }
        }
    }
    result.sort_by(|a, b| a.runtime_id.cmp(&b.runtime_id));
    Ok(result)
}

pub async fn remove_java_runtime(root: &Path, runtime_id: &str) -> Result<(), String> {
    let (component, version) = runtime_id
        .split_once('@')
        .ok_or("Invalid Java runtime ID")?;
    if !safe_component(component)
        || !safe_component(version)
        || !component.starts_with("java-runtime-") && component != "jre-legacy"
    {
        return Err("Invalid Java runtime ID".into());
    }
    let path = root.join("runtimes").join(component).join(version);
    ensure_no_symlink(root, &path).await?;
    tokio::fs::remove_dir_all(path)
        .await
        .map_err(|e| e.to_string())
}

/// Identity used for a game launch. Online tokens stay in memory and are never
/// included in process records or logs.
pub(super) enum LaunchIdentity {
    Offline {
        username: String,
    },
    Online {
        username: String,
        uuid: String,
        access_token: String,
        xuid: Option<String>,
        client_id: String,
    },
}

struct ResolvedIdentity {
    username: String,
    uuid: String,
    access_token: String,
    user_type: &'static str,
    xuid: String,
    client_id: String,
}

fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 16
        && username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

impl LaunchIdentity {
    pub(super) fn offline(username: &str) -> Result<Self, String> {
        if !valid_username(username) {
            return Err(
                "Offline username must be 1–16 ASCII letters, digits, or underscores".into(),
            );
        }
        Ok(Self::Offline {
            username: username.to_owned(),
        })
    }

    fn resolve(self) -> Result<ResolvedIdentity, String> {
        match self {
            Self::Offline { username } => {
                // Match Minecraft's standard offline profile UUID derivation
                // without shelling out.
                let mut offline_uuid =
                    md5::compute(format!("OfflinePlayer:{username}").as_bytes()).0;
                offline_uuid[6] = (offline_uuid[6] & 0x0f) | 0x30;
                offline_uuid[8] = (offline_uuid[8] & 0x3f) | 0x80;
                Ok(ResolvedIdentity {
                    username,
                    uuid: Uuid::from_bytes(offline_uuid).to_string(),
                    access_token: "offline".into(),
                    user_type: "legacy",
                    xuid: String::new(),
                    client_id: String::new(),
                })
            }
            Self::Online {
                username,
                uuid,
                access_token,
                xuid,
                client_id,
            } => {
                if !valid_username(&username) {
                    return Err("The signed-in Minecraft profile has an invalid username".into());
                }
                let uuid = uuid.replace('-', "").to_ascii_lowercase();
                if uuid.len() != 32 || !uuid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err("The signed-in Minecraft profile has an invalid UUID".into());
                }
                if access_token.is_empty() {
                    return Err("The Minecraft access token is empty".into());
                }
                Ok(ResolvedIdentity {
                    username,
                    uuid,
                    access_token,
                    user_type: "msa",
                    xuid: xuid.unwrap_or_default(),
                    client_id,
                })
            }
        }
    }
}

pub async fn launch_vanilla(
    root: &Path,
    id: &str,
    identity: LaunchIdentity,
    java_path: Option<&str>,
    extra_jvm_args: &[String],
    extra_game_args: &[String],
    memory_override: Option<MemoryConfig>,
    resolution: Option<(u32, u32)>,
    fullscreen: bool,
) -> Result<GameProcess, String> {
    let identity = identity.resolve()?;
    let cfg = read_config(root, id).await?;
    if cfg.loader.is_some() {
        return Err("This launcher currently supports vanilla instances only".into());
    }
    let state_path = root.join("instances").join(id).join("install.json");
    let state: Value = serde_json::from_slice(
        &tokio::fs::read(&state_path)
            .await
            .map_err(|_| "Instance is not installed; call instance.install first")?,
    )
    .map_err(|e| e.to_string())?;
    let metadata_path = root
        .join("versions")
        .join(&cfg.minecraft_version)
        .join(format!("{}.json", cfg.minecraft_version));
    let metadata: Value = serde_json::from_slice(
        &tokio::fs::read(metadata_path)
            .await
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let required = metadata["javaVersion"]["majorVersion"]
        .as_u64()
        .unwrap_or(8) as u32;
    let component = metadata["javaVersion"]["component"].as_str();
    let embedded = launcher::embedded_config();
    let java = if let Some(config) = &embedded {
        // No `java` executable exists in the app process; the embedded backend
        // uses the JRE directory directly.
        JavaRuntimeInfo {
            path: config
                .java_home
                .join("bin")
                .join("java")
                .to_string_lossy()
                .into_owned(),
            major_version: 21,
            vendor_version: "OpenJDK 21 (embedded)".into(),
            compatible: 21 >= required,
            required_major: required,
            runtime_id: None,
        }
    } else if let Some(path) = java_path {
        detect_java(Some(path), required)?
    } else {
        let managed = list_java_runtimes(root).await?.into_iter().find(|runtime| {
            runtime.compatible
                && runtime.major_version >= required
                && component.is_some_and(|component| {
                    runtime
                        .runtime_id
                        .as_deref()
                        .is_some_and(|id| id.starts_with(&format!("{component}@")))
                })
        });
        match managed {
            Some(runtime) => runtime,
            None => detect_java(None, required)?,
        }
    };
    if !java.compatible {
        return Err(format!(
            "Java {} is incompatible with Minecraft {} (requires Java {} or newer). Install its managed runtime with `java.runtime.install` or select a compatible Java.",
            java.major_version, cfg.minecraft_version, required
        ));
    }
    let game_dir = root.join("instances").join(id).join("game");
    ensure_no_symlink(root, &game_dir).await?;
    tokio::fs::create_dir_all(&game_dir)
        .await
        .map_err(|e| e.to_string())?;
    let cp = state["classpath"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(if cfg!(target_os = "windows") {
            ";"
        } else {
            ":"
        });
    let assets = root.join("assets");
    let mut jvm_args: Vec<String> = Vec::new();
    let memory = memory_override.as_ref().unwrap_or(&cfg.memory);
    if memory.minimum_mib == 0
        || memory.maximum_mib < memory.minimum_mib
        || memory.maximum_mib > 1_048_576
    {
        return Err("Invalid memory range in MiB".into());
    }
    if resolution.is_some_and(|(w, h)| w == 0 || h == 0 || w > 16384 || h > 16384) {
        return Err("Resolution must be between 1 and 16384 pixels".into());
    }
    let mut launch_features = std::collections::HashMap::new();
    launch_features.insert("is_demo_user", false);
    launch_features.insert("has_custom_resolution", resolution.is_some());
    launch_features.insert("has_quick_plays_support", false);
    jvm_args.push(format!("-Xms{}M", memory.minimum_mib));
    jvm_args.push(format!("-Xmx{}M", memory.maximum_mib));
    if extra_jvm_args
        .iter()
        .chain(extra_game_args)
        .any(|arg| arg.contains('\0'))
    {
        return Err("Launch arguments may not contain NUL bytes".into());
    }
    let process_id = Uuid::new_v4().to_string();
    jvm_args.extend(extra_jvm_args.iter().cloned());
    jvm_args.push(format!("-Dumc.process_id={process_id}"));
    let native_dir = root.join("natives").join(&cfg.minecraft_version);
    jvm_args.push(format!("-Djava.library.path={}", native_dir.display()));
    if let Some(jvm) = metadata["arguments"]["jvm"].as_array() {
        let mut flattened = Vec::new();
        for item in jvm {
            if let Some(s) = item.as_str() {
                flattened.push(s.to_owned());
            } else if rules_allow(item.get("rules"), &launch_features) {
                match &item["value"] {
                    Value::String(s) => flattened.push(s.clone()),
                    Value::Array(a) => {
                        flattened.extend(a.iter().filter_map(Value::as_str).map(str::to_owned))
                    }
                    _ => {}
                }
            }
        }
        let mut iter = flattened.into_iter();
        while let Some(mut arg) = iter.next() {
            if arg == "-cp" || arg == "-classpath" {
                let _ = iter.next();
                continue;
            }
            arg = arg
                .replace("${classpath}", &cp)
                .replace("${natives_directory}", &native_dir.to_string_lossy())
                .replace(
                    "${library_directory}",
                    &root.join("libraries").to_string_lossy(),
                )
                .replace("${launcher_name}", "UMC")
                .replace("${launcher_version}", env!("CARGO_PKG_VERSION"))
                .replace("${version_name}", &cfg.minecraft_version)
                .replace("${path}", state["log_config"].as_str().unwrap_or(""));
            if !arg.contains("${") {
                jvm_args.push(arg);
            }
        }
    }
    let main_class = metadata["mainClass"]
        .as_str()
        .ok_or("Missing Minecraft main class")?
        .to_owned();
    let token_uuid = identity.uuid.clone();
    let auth_session = if identity.access_token == "offline" {
        "token:offline:00000000-0000-0000-0000-000000000000".to_owned()
    } else {
        format!("token:{}:{}", identity.access_token, identity.uuid)
    };
    let mut game_args = vec![
        "--username".into(),
        identity.username.clone(),
        "--version".into(),
        cfg.minecraft_version.clone(),
        "--gameDir".into(),
        game_dir.to_string_lossy().into_owned(),
        "--assetsDir".into(),
        assets.to_string_lossy().into_owned(),
        "--assetIndex".into(),
        state["assets_index"].as_str().unwrap_or("").into(),
        "--uuid".into(),
        token_uuid.clone(),
        "--accessToken".into(),
        identity.access_token.clone(),
        "--userType".into(),
        identity.user_type.into(),
        "--versionType".into(),
        "UMC".into(),
    ];
    if let Some((width, height)) = resolution {
        game_args.extend([
            "--width".into(),
            width.to_string(),
            "--height".into(),
            height.to_string(),
        ]);
    }
    if fullscreen {
        game_args.push("--fullscreen".into());
    }
    if let Some(args) = metadata["arguments"]["game"].as_array() {
        game_args.clear();
        for arg in args {
            if let Some(s) = arg.as_str() {
                game_args.push(s.to_owned());
            } else if rules_allow(arg.get("rules"), &launch_features) {
                match &arg["value"] {
                    Value::String(s) => game_args.push(s.clone()),
                    Value::Array(a) => {
                        game_args.extend(a.iter().filter_map(Value::as_str).map(str::to_owned))
                    }
                    _ => {}
                }
            }
        }
        for arg in &mut game_args {
            *arg = arg
                .replace("${auth_player_name}", &identity.username)
                .replace("${version_name}", &cfg.minecraft_version)
                .replace("${game_directory}", &game_dir.to_string_lossy())
                .replace("${assets_root}", &assets.to_string_lossy())
                .replace(
                    "${game_assets}",
                    &assets
                        .join("virtual")
                        .join(state["virtual_assets"].as_str().unwrap_or(""))
                        .to_string_lossy(),
                )
                .replace(
                    "${assets_index_name}",
                    state["assets_index"].as_str().unwrap_or(""),
                )
                .replace("${auth_uuid}", &token_uuid)
                .replace("${auth_access_token}", &identity.access_token)
                .replace("${auth_session}", &auth_session)
                .replace("${auth_xuid}", &identity.xuid)
                .replace("${clientid}", &identity.client_id)
                .replace("${user_properties}", "{}")
                .replace("${user_type}", identity.user_type)
                .replace("${version_type}", "UMC")
                .replace(
                    "${resolution_width}",
                    &resolution.map(|v| v.0).unwrap_or(854).to_string(),
                )
                .replace(
                    "${resolution_height}",
                    &resolution.map(|v| v.1).unwrap_or(480).to_string(),
                )
                .replace("${classpath}", &cp)
                .replace(
                    "${natives_directory}",
                    &root
                        .join("natives")
                        .join(&cfg.minecraft_version)
                        .to_string_lossy(),
                );
        }
    } else if let Some(legacy) = metadata["minecraftArguments"].as_str() {
        game_args = shell_words::split(legacy)
            .map_err(|e| format!("Invalid legacy game arguments: {e}"))?;
        for arg in &mut game_args {
            *arg = arg
                .replace("${auth_player_name}", &identity.username)
                .replace("${version_name}", &cfg.minecraft_version)
                .replace("${game_directory}", &game_dir.to_string_lossy())
                .replace("${assets_root}", &assets.to_string_lossy())
                .replace(
                    "${game_assets}",
                    &assets
                        .join("virtual")
                        .join(state["virtual_assets"].as_str().unwrap_or(""))
                        .to_string_lossy(),
                )
                .replace(
                    "${assets_index_name}",
                    state["assets_index"].as_str().unwrap_or(""),
                )
                .replace("${auth_uuid}", &token_uuid)
                .replace("${auth_access_token}", &identity.access_token)
                .replace("${auth_session}", &auth_session)
                .replace("${auth_xuid}", &identity.xuid)
                .replace("${clientid}", &identity.client_id)
                .replace("${user_properties}", "{}")
                .replace("${user_type}", identity.user_type)
                .replace("${version_type}", "UMC");
        }
    }
    game_args.extend_from_slice(extra_game_args);
    let log_dir = root.join("instances").join(id).join("logs");
    ensure_no_symlink(root, &log_dir).await?;
    tokio::fs::create_dir_all(&log_dir)
        .await
        .map_err(|e| e.to_string())?;
    let spec = JavaLaunchSpec {
        java: PathBuf::from(&java.path),
        class_path: cp,
        main_class,
        jvm_args,
        game_args,
        work_dir: game_dir,
        console_log: log_dir.join("umc-console.log"),
        backend: match &embedded {
            Some(config) => LaunchBackend::Embedded(config.clone()),
            None => LaunchBackend::Process,
        },
    };
    let started = launcher::start(&spec)?;
    let embedded_process = started.is_embedded();
    let process = GameProcess {
        process_id,
        instance_id: id.into(),
        pid: started.pid(),
        kind: if embedded_process {
            ProcessKind::Embedded
        } else {
            ProcessKind::Process
        },
        status: ProcessStatus::Running,
        exit_code: None,
        started_at: timestamp_rfc3339(),
    };
    let process_dir = root.join("processes");
    ensure_no_symlink(root, &process_dir).await?;
    tokio::fs::create_dir_all(&process_dir)
        .await
        .map_err(|e| e.to_string())?;
    set_private_dir(&process_dir).await?;
    let process_file = process_dir.join(format!("{}.json", process.process_id));
    write_process_record(&process_file, &process)?;
    let process_for_monitor = process.clone();
    std::thread::spawn(move || {
        let mut state = process_for_monitor;
        match started {
            StartedJvm::Child(mut child) => loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        state.status = ProcessStatus::Exited;
                        state.exit_code = status.code();
                        let _ = write_process_record(&process_file, &state);
                        break;
                    }
                    Err(_) => {
                        state.status = ProcessStatus::Failed;
                        let _ = write_process_record(&process_file, &state);
                        break;
                    }
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(400)),
                }
            },
            StartedJvm::Embedded => loop {
                match launcher::embedded_status() {
                    Some(launcher::MAIN_RUNNING) => {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                    }
                    Some(launcher::MAIN_FINISHED) => {
                        state.status = ProcessStatus::Exited;
                        state.exit_code = Some(0);
                        let _ = write_process_record(&process_file, &state);
                        break;
                    }
                    Some(launcher::MAIN_FAILED) => {
                        state.status = ProcessStatus::Failed;
                        state.exit_code = Some(-1);
                        let _ = write_process_record(&process_file, &state);
                        break;
                    }
                    // The JVM is gone (e.g. the host process restarted).
                    _ => {
                        state.status = ProcessStatus::Exited;
                        state.exit_code = None;
                        let _ = write_process_record(&process_file, &state);
                        break;
                    }
                }
            },
        }
    });
    Ok(process)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_manifest_paths_cannot_escape_runtime_root() {
        assert!(safe_runtime_relative_path("bin/java").is_ok());
        assert!(safe_runtime_relative_path("../outside").is_err());
        assert!(safe_runtime_relative_path("/absolute/path").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn java_compatibility_reports_installed_but_too_old_runtime() {
        use std::os::unix::fs::PermissionsExt;
        let root = test_root("java-compatibility");
        std::fs::create_dir_all(&root).unwrap();
        let java = root.join("java");
        std::fs::write(
            &java,
            b"#!/bin/sh\necho 'openjdk version \"1.8.0_402\"' >&2\nexit 0\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&java).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&java, permissions).unwrap();

        let detected = detect_java(Some(java.to_str().unwrap()), 17).unwrap();
        assert_eq!(detected.major_version, 8);
        assert_eq!(detected.required_major, 17);
        assert!(!detected.compatible);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("umc-{name}-{}", Uuid::new_v4()))
    }

    fn sample_config(id: &str) -> InstanceConfig {
        InstanceConfig {
            schema_version: INSTANCE_CONFIG_SCHEMA_VERSION,
            id: id.into(),
            name: "Test instance".into(),
            minecraft_version: "1.21.11".into(),
            loader: None,
            java_runtime: "auto".into(),
            memory: MemoryConfig {
                minimum_mib: 1024,
                maximum_mib: 4096,
            },
            game_directory: format!("instances/{id}/game"),
        }
    }

    #[tokio::test]
    async fn legacy_instance_config_migrates_and_keeps_backup() {
        let root = test_root("migration");
        let dir = root.join("instances/legacy");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut legacy = serde_json::to_value(sample_config("legacy")).unwrap();
        legacy.as_object_mut().unwrap().remove("schema_version");
        tokio::fs::write(
            dir.join("instance.json"),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .await
        .unwrap();

        let migrated = read_config(&root, "legacy").await.unwrap();
        assert_eq!(migrated.schema_version, INSTANCE_CONFIG_SCHEMA_VERSION);
        assert!(dir.join("instance.json.bak").exists());
        let on_disk: InstanceConfig =
            serde_json::from_slice(&tokio::fs::read(dir.join("instance.json")).await.unwrap())
                .unwrap();
        assert_eq!(on_disk.schema_version, INSTANCE_CONFIG_SCHEMA_VERSION);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn corrupt_primary_recovers_from_backup_and_is_preserved() {
        let root = test_root("backup-recovery");
        let dir = root.join("instances/recover");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let original = sample_config("recover");
        write_config(&root, &dir.join("instance.json"), &original)
            .await
            .unwrap();
        let mut newer = original.clone();
        newer.name = "Newer name".into();
        write_config(&root, &dir.join("instance.json"), &newer)
            .await
            .unwrap();
        tokio::fs::write(dir.join("instance.json"), b"{broken json")
            .await
            .unwrap();

        let recovered = read_config(&root, "recover").await.unwrap();
        assert_eq!(recovered.name, original.name);
        let files = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            files
                .iter()
                .any(|f| f.starts_with("instance.json.corrupt-"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn repair_reports_backup_as_recovery_source() {
        let root = test_root("repair-backup-source");
        let dir = root.join("instances/recover");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let backup = sample_config("recover");
        tokio::fs::write(
            dir.join("instance.json.bak"),
            serde_json::to_vec(&backup).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(dir.join("instance.json"), b"broken")
            .await
            .unwrap();

        let result = repair_instance(&root, "recover").await.unwrap();
        assert_eq!(result.recovered_from, "backup");
        assert_eq!(result.config, backup);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn repair_rebuilds_config_from_valid_install_record() {
        let root = test_root("repair");
        let dir = root.join("instances/repair-me");
        let version_dir = root.join("versions/1.21.11");
        tokio::fs::create_dir_all(dir.join("game/saves/world"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(&version_dir).await.unwrap();
        tokio::fs::write(dir.join("game/saves/world/level.dat"), b"keep this world")
            .await
            .unwrap();
        tokio::fs::write(
            version_dir.join("1.21.11.json"),
            br#"{"id":"1.21.11","mainClass":"net.minecraft.client.main.Main"}"#,
        )
        .await
        .unwrap();
        let classpath = root
            .join("versions/1.21.11/1.21.11.jar")
            .to_string_lossy()
            .into_owned();
        tokio::fs::write(dir.join("install.json"), serde_json::to_vec(&serde_json::json!({"version":"1.21.11","main_class":"net.minecraft.client.main.Main","classpath":[classpath]})).unwrap()).await.unwrap();
        tokio::fs::write(dir.join("instance.json"), b"bad")
            .await
            .unwrap();
        tokio::fs::write(dir.join("instance.json.bak"), b"also bad")
            .await
            .unwrap();

        let result = repair_instance(&root, "repair-me").await.unwrap();
        assert_eq!(result.recovered_from, "installation_record");
        assert_eq!(result.config.minecraft_version, "1.21.11");
        assert_eq!(
            read_config(&root, "repair-me").await.unwrap(),
            result.config
        );
        assert_eq!(
            tokio::fs::read(dir.join("game/saves/world/level.dat"))
                .await
                .unwrap(),
            b"keep this world"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn removal_refuses_running_process_and_removes_exited_instance() {
        let root = test_root("remove-guard");
        let id = "remove-me";
        let dir = root.join("instances").join(id);
        tokio::fs::create_dir_all(dir.join("game")).await.unwrap();
        write_config(&root, &dir.join("instance.json"), &sample_config(id))
            .await
            .unwrap();
        let process_dir = root.join("processes");
        tokio::fs::create_dir_all(&process_dir).await.unwrap();
        let process_path = process_dir.join("running.json");
        let process = GameProcess {
            process_id: "running".into(),
            instance_id: id.into(),
            pid: 123,
            kind: ProcessKind::Process,
            status: ProcessStatus::Running,
            started_at: "2026-01-01T00:00:00Z".into(),
            exit_code: None,
        };
        tokio::fs::write(&process_path, serde_json::to_vec(&process).unwrap())
            .await
            .unwrap();

        assert!(
            remove_instance(&root, id)
                .await
                .unwrap_err()
                .contains("Stop this instance")
        );
        assert!(dir.exists());
        let mut exited = process;
        exited.status = ProcessStatus::Exited;
        exited.exit_code = Some(0);
        tokio::fs::write(&process_path, serde_json::to_vec(&exited).unwrap())
            .await
            .unwrap();
        remove_instance(&root, id).await.unwrap();
        assert!(!dir.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn corrupt_process_record_fails_closed_during_removal() {
        let root = test_root("corrupt-process");
        let id = "keep-me";
        let dir = root.join("instances").join(id);
        tokio::fs::create_dir_all(dir.join("game")).await.unwrap();
        write_config(&root, &dir.join("instance.json"), &sample_config(id))
            .await
            .unwrap();
        let processes = root.join("processes");
        tokio::fs::create_dir_all(&processes).await.unwrap();
        tokio::fs::write(processes.join("unknown.json"), b"broken")
            .await
            .unwrap();

        assert!(
            remove_instance(&root, id)
                .await
                .unwrap_err()
                .contains("Corrupt game process state")
        );
        assert!(dir.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn listing_surfaces_corrupt_config_without_deleting_data() {
        let root = test_root("list-corrupt");
        let dir = root.join("instances/broken");
        tokio::fs::create_dir_all(dir.join("game")).await.unwrap();
        tokio::fs::write(dir.join("instance.json"), b"not json")
            .await
            .unwrap();
        let error = list_instances(&root).await.unwrap_err();
        assert!(error.contains("broken"));
        assert!(dir.join("instance.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn future_schema_is_not_downgraded() {
        let root = test_root("future-schema");
        let dir = root.join("instances/future");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut config = sample_config("future");
        config.schema_version = INSTANCE_CONFIG_SCHEMA_VERSION + 1;
        let original = serde_json::to_vec(&config).unwrap();
        let path = dir.join("instance.json");
        tokio::fs::write(&path, &original).await.unwrap();
        let error = read_config(&root, "future").await.unwrap_err();
        assert!(error.contains("newer than this Core"));
        assert_eq!(tokio::fs::read(path).await.unwrap(), original);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn asset_file_digest_checks_content_and_size() {
        let root = test_root("asset-integrity");
        tokio::fs::create_dir_all(&root).await.unwrap();
        let file = root.join("asset.bin");
        tokio::fs::write(&file, b"abc").await.unwrap();
        let (hash, size) = file_sha1(&file).await.unwrap();
        assert_eq!(hash, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(size, 3);
        assert!(verify_file(&file, "a9993e364706816aba3e25717850c26c9cd0d89d", Some(3)).await);
        assert!(!verify_file(&file, "a9993e364706816aba3e25717850c26c9cd0d89d", Some(4)).await);
        assert!(!verify_file(&file, "0000000000000000000000000000000000000000", Some(3)).await);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installer_asset_index_validation_deduplicates_and_rejects_bad_hashes() {
        let hash = "a9993e364706816aba3e25717850c26c9cd0d89d";
        let index = serde_json::json!({"objects":{"one":{"hash":hash,"size":3},"two":{"hash":hash,"size":3}}});
        let assets = collect_asset_objects(&index).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[hash], 3);
        let invalid = serde_json::json!({"objects":{"bad":{"hash":"../oops","size":3}}});
        assert!(collect_asset_objects(&invalid).is_err());
        let conflicting = serde_json::json!({"objects":{"one":{"hash":hash,"size":3},"two":{"hash":hash,"size":4}}});
        assert!(collect_asset_objects(&conflicting).is_err());
    }

    #[test]
    fn installer_rejects_non_https_sources_and_traversal_paths() {
        assert!(validate_download_url("https://example.invalid/file").is_ok());
        assert!(validate_download_url("http://example.invalid/file").is_err());
        let root = Path::new("/tmp/umc-test-root");
        assert!(safe_relative(root, "libraries/example.jar").is_ok());
        assert!(safe_relative(root, "../outside.jar").is_err());
    }

    #[test]
    fn config_validation_rejects_paths_and_invalid_memory() {
        let mut cfg = sample_config("safe");
        cfg.game_directory = "instances/../outside".into();
        assert!(parse_config(&serde_json::to_vec(&cfg).unwrap(), "safe").is_err());
        cfg = sample_config("safe");
        cfg.memory.maximum_mib = 512;
        assert!(parse_config(&serde_json::to_vec(&cfg).unwrap(), "safe").is_err());
    }

    #[test]
    fn online_identity_uses_profile_values() {
        let identity = LaunchIdentity::Online {
            username: "Player_1".into(),
            uuid: "069A79F4-44E9-4726-A5BE-FCA90E38AAF5".into(),
            access_token: "mc-token".into(),
            xuid: Some("2535400000000000".into()),
            client_id: "client-id".into(),
        }
        .resolve()
        .unwrap();
        assert_eq!(identity.username, "Player_1");
        assert_eq!(identity.uuid, "069a79f444e94726a5befca90e38aaf5");
        assert_eq!(identity.access_token, "mc-token");
        assert_eq!(identity.user_type, "msa");
        assert_eq!(identity.xuid, "2535400000000000");
        assert_eq!(identity.client_id, "client-id");
    }

    #[test]
    fn offline_identity_derives_standard_uuid() {
        let identity = LaunchIdentity::offline("Steve").unwrap().resolve().unwrap();
        assert_eq!(identity.uuid, "5627dd98-e6be-3c21-b8a8-e92344183641");
        assert_eq!(identity.access_token, "offline");
        assert_eq!(identity.user_type, "legacy");
        assert!(LaunchIdentity::offline("bad name").is_err());
    }
}
