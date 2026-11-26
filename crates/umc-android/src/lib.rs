//! JNI bridge that embeds UMC Core inside an Android application.
//!
//! The Java side (`dev.umc.mobile.jvmdemo.UmcCoreBridge`) loads this library and
//! calls it from a background thread. All calls create a small Tokio runtime,
//! run the async core operation and return JSON.

use jni::objects::{JClass, JObjectArray, JString};
use jni::sys::{jint, jstring};
use jni::JNIEnv;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::{Duration, Instant};

fn into_jstring(env: &mut JNIEnv, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns the UMC Core version string.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_umc_mobile_jvmdemo_UmcCoreBridge_nativeVersion(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    into_jstring(&mut env, env!("CARGO_PKG_VERSION"))
}

/// Lists available Minecraft versions as JSON. `data_dir` is the app-private
/// directory where UMC keeps its metadata and instances.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_umc_mobile_jvmdemo_UmcCoreBridge_nativeVersions(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) -> jstring {
    let dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(e) => return into_jstring(&mut env, &format!("ERROR: invalid data dir: {e}")),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return into_jstring(&mut env, &format!("ERROR: runtime: {e}")),
    };

    let result = runtime.block_on(async move {
        let launcher = umc_sdk::Launcher::builder()
            .data_dir(dir)
            .build()
            .await
            .map_err(|e| format!("build launcher: {e}"))?;
        launcher
            .versions()
            .await
            .map_err(|e| format!("list versions: {e}"))
    });

    match result {
        Ok(versions) => {
            let json = serde_json::to_string(&versions)
                .unwrap_or_else(|e| format!("ERROR: serialize: {e}"));
            into_jstring(&mut env, &json)
        }
        Err(e) => into_jstring(&mut env, &format!("ERROR: {e}")),
    }
}

/// Diagnostic: resolves a host and connects to every resolved address.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_umc_mobile_jvmdemo_UmcCoreBridge_nativeTcpProbe(
    mut env: JNIEnv,
    _class: JClass,
    host: JString,
    port: jint,
) -> jstring {
    let host: String = match env.get_string(&host) {
        Ok(s) => s.into(),
        Err(e) => return into_jstring(&mut env, &format!("ERROR: host: {e}")),
    };
    let port = port as u16;
    let mut out = String::new();

    match (host.as_str(), port).to_socket_addrs() {
        Ok(addrs) => {
            let addrs: Vec<_> = addrs.collect();
            out.push_str(&format!("dns={} addr(s)", addrs.len()));
            for addr in addrs.iter().take(4) {
                match std::net::TcpStream::connect_timeout(addr, Duration::from_secs(5)) {
                    Ok(_) => out.push_str(&format!("; {addr}=ok")),
                    Err(e) => out.push_str(&format!("; {addr}=err({e})")),
                }
            }
        }
        Err(e) => out.push_str(&format!("dns error: {e}")),
    }

    into_jstring(&mut env, &out)
}

/// Diagnostic: performs an HTTPS GET with reqwest and reports the error chain.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_umc_mobile_jvmdemo_UmcCoreBridge_nativeHttpGet(
    mut env: JNIEnv,
    _class: JClass,
    url: JString,
) -> jstring {
    let url: String = match env.get_string(&url) {
        Ok(s) => s.into(),
        Err(e) => return into_jstring(&mut env, &format!("ERROR: url: {e}")),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return into_jstring(&mut env, &format!("ERROR: runtime: {e}")),
    };

    let result = runtime.block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("client: {e}"))?;
        match client.get(&url).send().await {
            Ok(resp) => Ok(format!("status {}", resp.status())),
            Err(e) => {
                let mut msg = format!("{e}");
                let mut source = std::error::Error::source(&e);
                while let Some(s) = source {
                    msg.push_str(" <- ");
                    msg.push_str(&s.to_string());
                    source = s.source();
                }
                Err(msg)
            }
        }
    });

    match result {
        Ok(v) => into_jstring(&mut env, &v),
        Err(e) => into_jstring(&mut env, &format!("ERROR: {e}")),
    }
}

fn read_jstring(env: &mut JNIEnv, value: &JString) -> Result<String, String> {
    env.get_string(value)
        .map(|s| s.into())
        .map_err(|e| e.to_string())
}

/// Diagnostic binding for the embedded launch backend: starts a JVM through
/// `umc-core`'s launch abstraction (`umc_jvm_create` / `umc_jvm_start_main`),
/// waits for `main` to finish and reports the status.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_umc_mobile_jvmdemo_UmcCoreBridge_nativeEmbeddedLaunch(
    mut env: JNIEnv,
    _class: JClass,
    java_home: JString,
    spawn_helper: JString,
    class_path: JString,
    main_class: JString,
    log_path: JString,
    options: JObjectArray,
) -> jstring {
    let result = (|| -> Result<String, String> {
        let java_home = read_jstring(&mut env, &java_home)?;
        let spawn_helper = read_jstring(&mut env, &spawn_helper)?;
        let class_path = read_jstring(&mut env, &class_path)?;
        let main_class = read_jstring(&mut env, &main_class)?;
        let log_path = read_jstring(&mut env, &log_path)?;

        let mut jvm_args = vec!["-Xmx256M".to_owned()];
        let count = env.get_array_length(&options).map_err(|e| e.to_string())?;
        for index in 0..count {
            let item = env
                .get_object_array_element(&options, index)
                .map_err(|e| e.to_string())?;
            let item = JString::from(item);
            jvm_args.push(read_jstring(&mut env, &item)?);
        }

        let spawn_helper = (!spawn_helper.is_empty()).then(|| Path::new(&spawn_helper));
        umc_core::start_embedded_jvm(
            Path::new(&java_home),
            spawn_helper,
            &class_path,
            &main_class,
            jvm_args,
            Vec::new(),
            Path::new(&log_path),
        )?;

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match umc_core::embedded_status() {
                Some(1) => return Ok("status=1".into()),
                Some(2) => return Err("main failed".into()),
                Some(status) if status == 0 || status == -1 => {
                    if Instant::now() >= deadline {
                        return Err("timed out waiting for main".into());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                other => return Err(format!("unexpected status {other:?}")),
            }
        }
    })();

    match result {
        Ok(value) => into_jstring(&mut env, &value),
        Err(error) => into_jstring(&mut env, &format!("ERROR: {error}")),
    }
}
