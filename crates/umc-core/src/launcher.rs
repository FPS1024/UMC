//! Launch abstraction: the engine builds one [`JavaLaunchSpec`] and the
//! platform backend decides how to run it.
//!
//! * Desktop: spawn the `java` binary and track its child process.
//! * Android: HotSpot is loaded into the launcher process; the spec is handed
//!   to the `libumc_jvm` C API instead of being executed as a subprocess.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Status codes returned by the embedded JVM, matching `umc_jvm.h`.
#[allow(dead_code)]
pub const MAIN_NOT_STARTED: i32 = -1;
pub const MAIN_RUNNING: i32 = 0;
pub const MAIN_FINISHED: i32 = 1;
pub const MAIN_FAILED: i32 = 2;

/// Configuration for the embedded (in-process) backend.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    pub java_home: PathBuf,
    /// Path to `jspawnhelper`, shipped as an APK native library.
    pub spawn_helper: Option<PathBuf>,
}

/// Selects how the JVM is started.
#[derive(Debug, Clone)]
pub enum LaunchBackend {
    Process,
    Embedded(EmbeddedConfig),
}

/// Everything needed to start a JVM running `main_class`.
#[derive(Debug, Clone)]
pub struct JavaLaunchSpec {
    /// The `java` executable (desktop only).
    pub java: PathBuf,
    /// ':'-separated class path.
    pub class_path: String,
    pub main_class: String,
    /// JVM options, without `-cp`.
    pub jvm_args: Vec<String>,
    /// Program arguments passed to `main`.
    pub game_args: Vec<String>,
    pub work_dir: PathBuf,
    /// stdout/stderr of the JVM are appended here.
    pub console_log: PathBuf,
    pub backend: LaunchBackend,
}

/// A JVM that has been started.
pub enum StartedJvm {
    Child(Child),
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Embedded,
}

impl StartedJvm {
    /// Operating-system pid, or 0 for an embedded JVM.
    pub fn pid(&self) -> u32 {
        match self {
            StartedJvm::Child(child) => child.id(),
            StartedJvm::Embedded => 0,
        }
    }

    pub fn is_embedded(&self) -> bool {
        matches!(self, StartedJvm::Embedded)
    }
}

/// Returns the embedded backend configuration when the host app provides one.
///
/// The app sets `UMC_EMBEDDED_JAVA_HOME` (or `UMC_JAVA_HOME`) to the extracted
/// JRE directory and `UMC_JSPAWNHELPER` to the packaged spawn helper.
pub fn embedded_config() -> Option<EmbeddedConfig> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let java_home = std::env::var("UMC_EMBEDDED_JAVA_HOME")
        .or_else(|_| std::env::var("UMC_JAVA_HOME"))
        .ok()
        .filter(|value| !value.is_empty())?;
    let spawn_helper = std::env::var("UMC_JSPAWNHELPER")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    Some(EmbeddedConfig {
        java_home: PathBuf::from(java_home),
        spawn_helper,
    })
}

/// Starts the JVM described by `spec`.
pub fn start(spec: &JavaLaunchSpec) -> Result<StartedJvm, String> {
    match &spec.backend {
        LaunchBackend::Process => start_process(spec),
        LaunchBackend::Embedded(config) => start_embedded(spec, config),
    }
}

fn start_process(spec: &JavaLaunchSpec) -> Result<StartedJvm, String> {
    if let Some(parent) = spec.console_log.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let stdout = std::fs::File::options()
        .create(true)
        .append(true)
        .open(&spec.console_log)
        .map_err(|e| e.to_string())?;
    let stderr = stdout.try_clone().map_err(|e| e.to_string())?;
    let child = Command::new(&spec.java)
        .args(&spec.jvm_args)
        .arg("-cp")
        .arg(&spec.class_path)
        .arg(&spec.main_class)
        .args(&spec.game_args)
        .current_dir(&spec.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| format!("Could not start Minecraft: {e}"))?;
    Ok(StartedJvm::Child(child))
}

/// Maximum heap from the last `-Xmx` option, in MiB.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn max_heap_mib(jvm_args: &[String]) -> u32 {
    for arg in jvm_args.iter().rev() {
        let Some(value) = arg.strip_prefix("-Xmx") else {
            continue;
        };
        let lower = value.to_ascii_lowercase();
        let (number, unit) = match lower.strip_suffix(|c: char| c.is_ascii_alphabetic()) {
            Some(number) => (number, lower.chars().last().unwrap_or('m')),
            None => (lower.as_str(), 'b'),
        };
        let Ok(size) = number.parse::<u64>() else {
            continue;
        };
        let mib = match unit {
            'k' => size / 1024,
            'm' => size,
            'g' => size.saturating_mul(1024),
            _ => size / (1024 * 1024),
        };
        return mib.clamp(16, 1_048_576) as u32;
    }
    2048
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn is_heap_option(arg: &str) -> bool {
    arg.starts_with("-Xmx") || arg.starts_with("-Xms")
}

#[cfg(target_os = "android")]
mod embedded {
    use super::*;
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    unsafe extern "C" {
        fn umc_jvm_create(
            java_home: *const c_char,
            class_path: *const c_char,
            max_heap_mb: c_int,
            options: *const *const c_char,
            option_count: c_int,
            spawn_helper_path: *const c_char,
        ) -> c_int;
        fn umc_jvm_start_main(
            main_class: *const c_char,
            args: *const *const c_char,
            arg_count: c_int,
        ) -> c_int;
        fn umc_jvm_main_status() -> c_int;
        fn umc_jvm_redirect_output(path: *const c_char);
        fn umc_jvm_is_created() -> c_int;
        fn umc_jvm_destroy();
    }

    fn cstring(value: &str) -> Result<CString, String> {
        CString::new(value).map_err(|_| "Launch arguments may not contain NUL bytes".to_string())
    }

    pub fn status() -> Option<i32> {
        // SAFETY: the C API is process-global and safe to poll from any thread.
        Some(unsafe { umc_jvm_main_status() })
    }

    pub fn start(spec: &JavaLaunchSpec, config: &EmbeddedConfig) -> Result<StartedJvm, String> {
        // SAFETY: no JVM may exist yet; the C API serializes creation itself.
        if unsafe { umc_jvm_is_created() } != 0 {
            return Err("A JVM is already running in this process".into());
        }

        let java_home = cstring(&config.java_home.to_string_lossy())?;
        let class_path = cstring(&spec.class_path)?;
        let console_log = cstring(&spec.console_log.to_string_lossy())?;
        let main_class = cstring(&spec.main_class)?;
        let spawn_helper = config
            .spawn_helper
            .as_ref()
            .map(|path| cstring(&path.to_string_lossy()))
            .transpose()?;

        let options: Vec<CString> = spec
            .jvm_args
            .iter()
            .filter(|arg| !is_heap_option(arg))
            .map(|arg| cstring(arg))
            .collect::<Result<_, _>>()?;
        let option_ptrs: Vec<*const c_char> = options.iter().map(|arg| arg.as_ptr()).collect();

        let args: Vec<CString> = spec
            .game_args
            .iter()
            .map(|arg| cstring(arg))
            .collect::<Result<_, _>>()?;
        let arg_ptrs: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();

        // SAFETY: all pointers are valid for the duration of the call.
        unsafe {
            umc_jvm_redirect_output(console_log.as_ptr());
            let rc = umc_jvm_create(
                java_home.as_ptr(),
                class_path.as_ptr(),
                max_heap_mib(&spec.jvm_args) as c_int,
                option_ptrs.as_ptr(),
                option_ptrs.len() as c_int,
                spawn_helper
                    .as_ref()
                    .map_or(std::ptr::null(), |path| path.as_ptr()),
            );
            if rc != 0 {
                return Err("Could not create the embedded JVM".into());
            }
            let rc = umc_jvm_start_main(
                main_class.as_ptr(),
                arg_ptrs.as_ptr(),
                arg_ptrs.len() as c_int,
            );
            if rc != 0 {
                umc_jvm_destroy();
                return Err("Could not start the embedded JVM main class".into());
            }
        }
        Ok(StartedJvm::Embedded)
    }

    pub fn destroy() {
        // SAFETY: safe to call from any thread, and a no-op without a JVM.
        unsafe { umc_jvm_destroy() }
    }
}

/// Current embedded JVM status, or `None` on platforms without the C API.
pub fn embedded_status() -> Option<i32> {
    #[cfg(target_os = "android")]
    {
        embedded::status()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Shuts down the embedded JVM, if any. Used when the host app exits.
pub fn shutdown_embedded() {
    #[cfg(target_os = "android")]
    {
        embedded::destroy();
    }
}

/// Starts an embedded JVM from explicit parameters.
///
/// Intended for Android hosts (such as UMC's app) that have already extracted
/// the JRE and do not dispatch through the instance engine. Returns once the
/// main thread has been scheduled; poll [`embedded_status`] for completion.
#[allow(clippy::too_many_arguments)]
pub fn start_embedded_jvm(
    java_home: &std::path::Path,
    spawn_helper: Option<&std::path::Path>,
    class_path: &str,
    main_class: &str,
    jvm_args: Vec<String>,
    game_args: Vec<String>,
    console_log: &std::path::Path,
) -> Result<(), String> {
    let spec = JavaLaunchSpec {
        java: java_home.join("bin").join("java"),
        class_path: class_path.to_owned(),
        main_class: main_class.to_owned(),
        jvm_args,
        game_args,
        work_dir: std::env::temp_dir(),
        console_log: console_log.to_owned(),
        backend: LaunchBackend::Embedded(EmbeddedConfig {
            java_home: java_home.to_owned(),
            spawn_helper: spawn_helper.map(std::path::Path::to_path_buf),
        }),
    };
    start(&spec).map(|_| ())
}

#[cfg(target_os = "android")]
fn start_embedded(spec: &JavaLaunchSpec, config: &EmbeddedConfig) -> Result<StartedJvm, String> {
    embedded::start(spec, config)
}

#[cfg(not(target_os = "android"))]
fn start_embedded(_spec: &JavaLaunchSpec, _config: &EmbeddedConfig) -> Result<StartedJvm, String> {
    Err("Embedded JVM launch is only available on Android".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_heap_suffixes() {
        assert_eq!(max_heap_mib(&["-Xmx512M".into()]), 512);
        assert_eq!(max_heap_mib(&["-Xmx2G".into()]), 2048);
        assert_eq!(max_heap_mib(&["-Xmx1048576K".into()]), 1024);
        assert_eq!(max_heap_mib(&[]), 2048);
    }
}
