//! The single business-logic implementation shared by SDK and executable adapters.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use umc_api::{
    CreateInstanceRequest, InstallRequest, JavaRuntimeInstallRequest, LaunchRequest, RpcError,
    RpcRequest, RpcResponse, UMC_API_VERSION, VersionManifest,
};
use umc_loader_api::{LoaderId, LoaderInstallRequest};
use umc_loader_registry::LoaderRegistry;
pub mod auth;
mod credentials;
mod daemon;
mod engine;
mod tasks;
use tasks::TaskManager;

pub async fn serve_local_daemon(core: UmcCore) -> std::io::Result<()> {
    daemon::serve_local(core).await
}

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

fn parse_loader_id(value: Option<&str>) -> Option<LoaderId> {
    LoaderId::new(value?).ok()
}

#[derive(Clone)]
pub struct UmcCore {
    data_dir: PathBuf,
    instance_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    tasks: TaskManager,
    loaders: LoaderRegistry,
    auth: auth::AuthManager,
}

impl UmcCore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self::with_loaders(data_dir, LoaderRegistry::new())
    }

    pub fn with_loaders(data_dir: impl Into<PathBuf>, loaders: LoaderRegistry) -> Self {
        let requested = data_dir.into();
        let data_dir = if requested.is_absolute() {
            requested
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(requested)
        };
        Self {
            tasks: TaskManager::new(data_dir.clone()),
            auth: auth::AuthManager::new(data_dir.clone(), auth::MicrosoftOAuthConfig::default()),
            loaders,
            data_dir,
            instance_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub async fn recover_processes(&self) -> Result<Vec<umc_api::GameProcess>, String> {
        self.tasks.recover().await?;
        engine::recover_processes(&self.data_dir).await
    }

    fn instance_lock(&self, id: &str) -> Result<Arc<tokio::sync::Mutex<()>>, String> {
        let mut locks = self
            .instance_locks
            .lock()
            .map_err(|_| "Instance lock map poisoned".to_string())?;
        Ok(locks
            .entry(id.to_owned())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone())
    }

    pub async fn dispatch(&self, req: RpcRequest) -> RpcResponse {
        let id = req.id.clone();
        if req.jsonrpc != "2.0" {
            return RpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(RpcError {
                    code: -32600,
                    message: "Invalid JSON-RPC version".into(),
                    data: None,
                }),
            };
        }
        let result = match req.method.as_str() {
            "system.info" => Ok(
                serde_json::json!({"name":"UMC","version":env!("CARGO_PKG_VERSION"),"api_versions":[UMC_API_VERSION]}),
            ),
            "system.capabilities" => Ok(
                serde_json::json!({"capabilities":["system","version.list","instance.create","instance.list","instance.get","instance.remove","instance.repair","instance.install","loader.list","loader.versions","loader.plan","java.detect","java.runtime.list","java.runtime.install","java.runtime.remove","launch.offline","launch.online","auth.status","auth.accounts","auth.login","auth.refresh","auth.logout","process.list","process.status","process.stop","task.get","task.cancel","task.events","rpc.stdio","rpc.local_daemon"]}),
            ),
            "loader.list" => {
                serde_json::to_value(self.loaders.available()).map_err(|e| (-32001, e.to_string()))
            }
            "loader.versions" => {
                let loader =
                    parse_loader_id(req.params.get("loader").and_then(serde_json::Value::as_str));
                let minecraft_version = req
                    .params
                    .get("minecraft_version")
                    .and_then(serde_json::Value::as_str);
                match (loader, minecraft_version) {
                    (Some(loader), Some(version)) => match self.loaders.get(&loader) {
                        Some(adapter) => adapter
                            .list_versions(version)
                            .await
                            .and_then(|versions| {
                                serde_json::to_value(versions).map_err(|e| e.to_string())
                            })
                            .map_err(|e| (-32005, e)),
                        None => Err((-32012, format!("Loader adapter {loader:?} is unavailable"))),
                    },
                    (None, _) => Err((
                        -32602,
                        "loader must be a valid registered provider id".into(),
                    )),
                    (_, None) => Err((-32602, "minecraft_version is required".into())),
                }
            }
            "loader.plan" => {
                let loader =
                    parse_loader_id(req.params.get("loader").and_then(serde_json::Value::as_str));
                let parsed = serde_json::from_value::<LoaderInstallRequest>(req.params.clone());
                match (loader, parsed) {
                    (Some(loader), Ok(request)) => match self.loaders.get(&loader) {
                        Some(adapter) => adapter
                            .plan_install(request)
                            .await
                            .and_then(|plan| serde_json::to_value(plan).map_err(|e| e.to_string()))
                            .map_err(|e| (-32005, e)),
                        None => Err((-32012, format!("Loader adapter {loader:?} is unavailable"))),
                    },
                    (None, _) => Err((
                        -32602,
                        "loader must be a valid registered provider id".into(),
                    )),
                    (_, Err(error)) => Err((-32602, error.to_string())),
                }
            }
            "version.list" => match self.version_manifest().await {
                Ok(manifest) => {
                    serde_json::to_value(manifest.versions).map_err(|e| (-32001, e.to_string()))
                }
                Err(e) => Err((-32005, e.to_string())),
            },
            "version.latest" => match self.version_manifest().await {
                Ok(m) => Ok(serde_json::to_value(m.latest).unwrap_or_default()),
                Err(e) => Err((-32005, e.to_string())),
            },
            "instance.create" => {
                let parsed = serde_json::from_value::<CreateInstanceRequest>(req.params.clone());
                match parsed {
                    Ok(p) => {
                        let id = format!("instance-{}", uuid::Uuid::new_v4().simple());
                        engine::create_instance(
                            &self.data_dir,
                            &id,
                            p.name,
                            p.minecraft_version,
                            p.memory,
                        )
                        .await
                        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                        .map_err(|e| (-32009, e))
                    }
                    Err(e) => Err((-32602, e.to_string())),
                }
            }
            "instance.list" => engine::list_instances(&self.data_dir)
                .await
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                .map_err(|e| (-32009, e)),
            "instance.get" => match req
                .params
                .get("instance_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => engine::get_instance(&self.data_dir, id)
                    .await
                    .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                    .map_err(|e| (-32009, e)),
                None => Err((-32602, "instance_id is required".into())),
            },
            "instance.repair" => match req
                .params
                .get("instance_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => match self.instance_lock(id) {
                    Ok(lock) => {
                        let _guard = lock.lock().await;
                        engine::repair_instance(&self.data_dir, id)
                            .await
                            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                            .map_err(|e| (-32009, e))
                    }
                    Err(e) => Err((-32001, e)),
                },
                None => Err((-32602, "instance_id is required".into())),
            },
            "instance.remove" => match req
                .params
                .get("instance_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => match self.instance_lock(id) {
                    Ok(lock) => {
                        let _guard = lock.lock().await;
                        engine::remove_instance(&self.data_dir, id)
                            .await
                            .map(|_| serde_json::json!({"instance_id":id,"status":"removed"}))
                            .map_err(|e| (-32009, e))
                    }
                    Err(e) => Err((-32001, e)),
                },
                None => Err((-32602, "instance_id is required".into())),
            },
            "process.status" => match req
                .params
                .get("process_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => engine::process_status(&self.data_dir, id)
                    .await
                    .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                    .map_err(|e| (-32003, e)),
                None => Err((-32602, "process_id is required".into())),
            },
            "process.list" => engine::list_processes(&self.data_dir)
                .await
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                .map_err(|e| (-32003, e)),
            "process.stop" => match req
                .params
                .get("process_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => engine::stop_process(&self.data_dir, id)
                    .await
                    .map(|_| serde_json::json!({"process_id":id,"status":"stopping"}))
                    .map_err(|e| (-32003, e)),
                None => Err((-32602, "process_id is required".into())),
            },
            "task.get" => match req
                .params
                .get("task_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => self
                    .tasks
                    .get(id)
                    .await
                    .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                    .map_err(|e| (-32003, e)),
                None => Err((-32602, "task_id is required".into())),
            },
            "task.cancel" => match req
                .params
                .get("task_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => self
                    .tasks
                    .cancel(id)
                    .await
                    .map(|_| serde_json::json!({"task_id":id,"status":"cancel_requested"}))
                    .map_err(|e| (-32003, e)),
                None => Err((-32602, "task_id is required".into())),
            },
            "task.events" => match req
                .params
                .get("task_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(id) => {
                    let after = req
                        .params
                        .get("after_sequence")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    let wait_ms = req
                        .params
                        .get("wait_ms")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    self.tasks
                        .events(id, after, wait_ms)
                        .await
                        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                        .map_err(|e| (-32003, e))
                }
                None => Err((-32602, "task_id is required".into())),
            },
            "instance.install" => {
                match serde_json::from_value::<InstallRequest>(req.params.clone()) {
                    Ok(p) => {
                        let core = self.clone();
                        let instance_id = p.instance_id;
                        let data_dir = self.data_dir.clone();
                        let timeout = p.timeout_seconds.unwrap_or(6 * 60 * 60);
                        self.tasks.spawn("instance.install", timeout, move |task| async move {
                            let lock = core.instance_lock(&instance_id)?;
                            let _guard = lock.lock().await;
                            engine::install_vanilla(&data_dir, &instance_id, &task).await?;
                            Ok(serde_json::json!({"instance_id":instance_id,"status":"installed"}))
                        }).await.and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())).map_err(|e| (-32602, e))
                    }
                    Err(e) => Err((-32602, e.to_string())),
                }
            }
            "java.detect" => match engine::detect_java(
                req.params.get("path").and_then(serde_json::Value::as_str),
                req.params
                    .get("minimum_major")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(8) as u32,
            ) {
                Ok(info) => serde_json::to_value(info).map_err(|e| (-32001, e.to_string())),
                Err(e) => Err((-32010, e)),
            },
            "java.runtime.list" => engine::list_java_runtimes(&self.data_dir)
                .await
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                .map_err(|e| (-32010, e)),
            "java.runtime.install" => {
                match serde_json::from_value::<JavaRuntimeInstallRequest>(req.params.clone()) {
                    Ok(request) => {
                        let data_dir = self.data_dir.clone();
                        let minecraft_version = request.minecraft_version;
                        let timeout = request.timeout_seconds.unwrap_or(6 * 60 * 60);
                        self.tasks
                            .spawn("java.runtime.install", timeout, move |task| async move {
                                let result = engine::install_java_runtime(
                                    &data_dir,
                                    &minecraft_version,
                                    &task,
                                )
                                .await?;
                                serde_json::to_value(result).map_err(|e| e.to_string())
                            })
                            .await
                            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                            .map_err(|e| (-32602, e))
                    }
                    Err(e) => Err((-32602, e.to_string())),
                }
            }
            "java.runtime.remove" => match req
                .params
                .get("runtime_id")
                .and_then(serde_json::Value::as_str)
            {
                Some(runtime_id) => engine::remove_java_runtime(&self.data_dir, runtime_id)
                    .await
                    .map(|_| serde_json::json!({"runtime_id":runtime_id,"status":"removed"}))
                    .map_err(|e| (-32010, e)),
                None => Err((-32602, "runtime_id is required".into())),
            },
            "auth.status" => self.auth.status().await.map_err(|e| (-32007, e)),
            "auth.accounts" => self
                .auth
                .accounts()
                .await
                .and_then(|accounts| serde_json::to_value(accounts).map_err(|e| e.to_string()))
                .map_err(|e| (-32007, e)),
            "auth.logout" => {
                let account_id = req
                    .params
                    .get("account_id")
                    .and_then(serde_json::Value::as_str);
                self.auth
                    .logout(account_id)
                    .await
                    .map(|account_id| {
                        serde_json::json!({"account_id":account_id,"status":"removed"})
                    })
                    .map_err(|e| (-32007, e))
            }
            "auth.refresh" => {
                let account_id = req
                    .params
                    .get("account_id")
                    .and_then(serde_json::Value::as_str);
                self.auth
                    .refresh_session(account_id, |_| {})
                    .await
                    .map(|session| {
                        serde_json::json!({
                            "account":{"id":session.profile.id,"name":session.profile.name},
                            "minecraft_expires_in":session.minecraft_expires_in,
                        })
                    })
                    .map_err(|e| (-32007, e))
            }
            "auth.login" => {
                let open_browser = req
                    .params
                    .get("open_browser")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let redirect_uri = req
                    .params
                    .get("redirect_uri")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                let auth = self.auth.clone();
                self.tasks
                    .spawn("auth.login", 15 * 60, move |task| async move {
                        let mut config = auth.config().clone();
                        if let Some(uri) = redirect_uri {
                            config.redirect_uri = uri;
                        }
                        let flow = auth::LoginFlow::start_with_config(config, open_browser).await?;
                        task.emit(
                            "auth.url",
                            serde_json::json!({
                                "url": flow.auth_url(),
                                "redirect_uri": flow.redirect_uri(),
                            }),
                        );
                        if let Some(error) = flow.browser_error() {
                            task.emit("auth.browser_error", serde_json::json!({"error": error}));
                        }
                        let oauth = flow.wait().await?;
                        let progress_task = task.clone();
                        let session =
                            auth::complete_minecraft_login_with_progress(&oauth, |stage| {
                                progress_task
                                    .emit("auth.stage", serde_json::json!({"stage": stage}));
                            })
                            .await?;
                        let (record, warning) = auth.save_session(&session).await?;
                        Ok(serde_json::json!({
                            "account": {"id": record.account_id, "name": record.username},
                            "storage_warning": warning,
                        }))
                    })
                    .await
                    .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
                    .map_err(|e| (-32007, e))
            }
            "launch.start" => {
                let request = match serde_json::from_value::<LaunchRequest>(req.params.clone()) {
                    Ok(request) => request,
                    Err(error) => {
                        return RpcResponse {
                            jsonrpc: "2.0".into(),
                            id,
                            result: None,
                            error: Some(RpcError {
                                code: -32602,
                                message: error.to_string(),
                                data: None,
                            }),
                        };
                    }
                };
                let identity_result: Result<engine::LaunchIdentity, (i32, String)> =
                    if request.offline_mode {
                        engine::LaunchIdentity::offline(&request.username).map_err(|e| (-32602, e))
                    } else {
                        self.auth
                            .refresh_session(request.account_id.as_deref(), |_| {})
                            .await
                            .map(|session| engine::LaunchIdentity::Online {
                                username: session.profile.name,
                                uuid: session.profile.id,
                                access_token: session.minecraft_access_token,
                                xuid: session.xuid,
                                client_id: self.auth.config().client_id.clone(),
                            })
                            .map_err(|e| (-32007, e))
                    };
                let identity = match identity_result {
                    Ok(identity) => identity,
                    Err((code, message)) => {
                        return RpcResponse {
                            jsonrpc: "2.0".into(),
                            id,
                            result: None,
                            error: Some(RpcError {
                                code,
                                message,
                                data: None,
                            }),
                        };
                    }
                };
                let instance_id = request.instance_id.clone();
                match self.instance_lock(&instance_id) {
                    Ok(lock) => {
                        let _guard = lock.lock().await;
                        let extra_jvm_args = request.extra_jvm_args;
                        let extra_game_args = request.extra_game_args;
                        engine::launch_vanilla(
                            &self.data_dir,
                            &instance_id,
                            identity,
                            request.java_path.as_deref(),
                            &extra_jvm_args,
                            &extra_game_args,
                            request.memory,
                            request.resolution.map(|value| (value.width, value.height)),
                            request.fullscreen,
                        )
                        .await
                        .and_then(|process| {
                            serde_json::to_value(process).map_err(|e| e.to_string())
                        })
                        .map_err(|e| (-32010, e))
                    }
                    Err(e) => Err((-32001, e)),
                }
            }
            _ => Err((-32601, format!("Method not found: {}", req.method))),
        };
        match result {
            Ok(value) => RpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: Some(value),
                error: None,
            },
            Err((code, message)) => RpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(RpcError {
                    code,
                    message,
                    data: None,
                }),
            },
        }
    }

    async fn version_manifest(&self) -> Result<VersionManifest, String> {
        let cache = self.data_dir.join("cache/version_manifest_v2.json");
        engine::ensure_no_symlink(&self.data_dir, &cache).await?;
        let cached = tokio::fs::read(&cache)
            .await
            .ok()
            .and_then(|raw| serde_json::from_slice::<VersionManifest>(&raw).ok());
        let fresh = tokio::fs::metadata(&cache)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() < 3600);
        if fresh {
            if let Some(manifest) = cached {
                return Ok(manifest);
            }
        }
        let response = engine::http_client()?
            .get(MANIFEST_URL)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string());
        let raw = match response {
            Ok(response) => response.bytes().await.map_err(|e| e.to_string())?,
            Err(error) => return cached.ok_or(error),
        };
        let manifest: VersionManifest = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        tokio::fs::create_dir_all(cache.parent().unwrap())
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::write(cache, &raw)
            .await
            .map_err(|e| e.to_string())?;
        Ok(manifest)
    }
}
