//! Stable Rust entry point. It delegates every operation to `umc-core`.
use std::{path::PathBuf, sync::Arc};
pub use umc_api::{
    AccountSummary, AuthRefreshResult, AuthStatus, CreateInstanceRequest, GameProcess,
    INSTANCE_CONFIG_SCHEMA_VERSION, InstallRequest, InstanceConfig, JavaRuntimeInfo,
    JavaRuntimeInstallRequest, JavaRuntimeInstallResult, LatestVersions, LaunchRequest,
    RepairInstanceResult, TaskEvent, TaskHandle, TaskInfo, TaskStatus, VersionEntry,
};
use umc_core::UmcCore;
use umc_loader_api::LoaderAdapter;
pub use umc_loader_api::{
    LoaderId, LoaderInstallPlan, LoaderInstallRequest, LoaderInstallStrategy, LoaderVersion,
};
use umc_loader_registry::LoaderRegistry;

#[derive(Clone)]
pub struct Launcher {
    core: UmcCore,
    data_dir: PathBuf,
    daemon: bool,
}
pub struct LauncherBuilder {
    data_dir: PathBuf,
    daemon: bool,
    loaders: LoaderRegistry,
}

impl Launcher {
    pub fn builder() -> LauncherBuilder {
        LauncherBuilder {
            data_dir: PathBuf::from("umc-data"),
            daemon: false,
            loaders: LoaderRegistry::new(),
        }
    }
    pub async fn versions(&self) -> Result<Vec<VersionEntry>, String> {
        serde_json::from_value(self.call("version.list", serde_json::Value::Null).await?)
            .map_err(|e| e.to_string())
    }
    pub async fn loaders(&self) -> Result<Vec<LoaderId>, String> {
        serde_json::from_value(self.call("loader.list", serde_json::Value::Null).await?)
            .map_err(|e| e.to_string())
    }
    pub async fn loader_versions(
        &self,
        loader: LoaderId,
        minecraft_version: &str,
    ) -> Result<Vec<LoaderVersion>, String> {
        serde_json::from_value(
            self.call(
                "loader.versions",
                serde_json::json!({"loader":loader,"minecraft_version":minecraft_version}),
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn plan_loader_install(
        &self,
        loader: LoaderId,
        request: LoaderInstallRequest,
    ) -> Result<LoaderInstallPlan, String> {
        serde_json::from_value(
            self.call(
                "loader.plan",
                serde_json::json!({"loader":loader,"minecraft_version":request.minecraft_version,"loader_version":request.loader_version}),
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn create_instance(
        &self,
        request: CreateInstanceRequest,
    ) -> Result<InstanceConfig, String> {
        serde_json::from_value(
            self.call(
                "instance.create",
                serde_json::to_value(request).map_err(|e| e.to_string())?,
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn instances(&self) -> Result<Vec<InstanceConfig>, String> {
        serde_json::from_value(self.call("instance.list", serde_json::Value::Null).await?)
            .map_err(|e| e.to_string())
    }
    pub async fn remove_instance(&self, instance_id: &str) -> Result<(), String> {
        self.call(
            "instance.remove",
            serde_json::json!({"instance_id":instance_id}),
        )
        .await?;
        Ok(())
    }
    pub async fn repair_instance(&self, instance_id: &str) -> Result<RepairInstanceResult, String> {
        serde_json::from_value(
            self.call(
                "instance.repair",
                serde_json::json!({"instance_id":instance_id}),
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn detect_java(
        &self,
        path: Option<&str>,
        minimum_major: u32,
    ) -> Result<JavaRuntimeInfo, String> {
        let mut params = serde_json::json!({"minimum_major":minimum_major});
        if let Some(path) = path {
            params["path"] = serde_json::Value::String(path.into());
        }
        serde_json::from_value(self.call("java.detect", params).await?).map_err(|e| e.to_string())
    }
    pub async fn java_runtimes(&self) -> Result<Vec<JavaRuntimeInfo>, String> {
        serde_json::from_value(
            self.call("java.runtime.list", serde_json::Value::Null)
                .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn install_java_runtime(
        &self,
        request: JavaRuntimeInstallRequest,
    ) -> Result<TaskHandle, String> {
        serde_json::from_value(
            self.call(
                "java.runtime.install",
                serde_json::to_value(request).map_err(|e| e.to_string())?,
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn remove_java_runtime(&self, runtime_id: &str) -> Result<(), String> {
        self.call(
            "java.runtime.remove",
            serde_json::json!({"runtime_id":runtime_id}),
        )
        .await?;
        Ok(())
    }
    pub async fn install(&self, request: InstallRequest) -> Result<TaskHandle, String> {
        serde_json::from_value(
            self.call(
                "instance.install",
                serde_json::to_value(request).map_err(|e| e.to_string())?,
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn launch(&self, request: LaunchRequest) -> Result<GameProcess, String> {
        serde_json::from_value(
            self.call(
                "launch.start",
                serde_json::to_value(request).map_err(|e| e.to_string())?,
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn auth_status(&self) -> Result<AuthStatus, String> {
        serde_json::from_value(self.call("auth.status", serde_json::Value::Null).await?)
            .map_err(|e| e.to_string())
    }
    pub async fn accounts(&self) -> Result<Vec<AccountSummary>, String> {
        serde_json::from_value(self.call("auth.accounts", serde_json::Value::Null).await?)
            .map_err(|e| e.to_string())
    }
    /// Starts a Microsoft sign-in task. Poll `auth.url` and `auth.stage` events
    /// with [`Launcher::task_events`] and read the result from [`Launcher::task`].
    pub async fn auth_login(
        &self,
        open_browser: bool,
        redirect_uri: Option<&str>,
    ) -> Result<TaskHandle, String> {
        let mut params = serde_json::json!({"open_browser": open_browser});
        if let Some(uri) = redirect_uri {
            params["redirect_uri"] = serde_json::Value::String(uri.to_owned());
        }
        serde_json::from_value(self.call("auth.login", params).await?).map_err(|e| e.to_string())
    }
    pub async fn auth_refresh(
        &self,
        account_id: Option<&str>,
    ) -> Result<AuthRefreshResult, String> {
        let params = match account_id {
            Some(id) => serde_json::json!({"account_id": id}),
            None => serde_json::json!({}),
        };
        serde_json::from_value(self.call("auth.refresh", params).await?).map_err(|e| e.to_string())
    }
    pub async fn auth_logout(&self, account_id: Option<&str>) -> Result<(), String> {
        let params = match account_id {
            Some(id) => serde_json::json!({"account_id": id}),
            None => serde_json::json!({}),
        };
        self.call("auth.logout", params).await?;
        Ok(())
    }
    pub async fn process_status(&self, process_id: &str) -> Result<GameProcess, String> {
        serde_json::from_value(
            self.call(
                "process.status",
                serde_json::json!({"process_id":process_id}),
            )
            .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn processes(&self) -> Result<Vec<GameProcess>, String> {
        serde_json::from_value(self.call("process.list", serde_json::Value::Null).await?)
            .map_err(|e| e.to_string())
    }
    pub async fn stop_process(&self, process_id: &str) -> Result<(), String> {
        self.call("process.stop", serde_json::json!({"process_id":process_id}))
            .await?;
        Ok(())
    }
    pub async fn task(&self, task_id: &str) -> Result<TaskInfo, String> {
        serde_json::from_value(
            self.call("task.get", serde_json::json!({"task_id":task_id}))
                .await?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn cancel_task(&self, task_id: &str) -> Result<(), String> {
        self.call("task.cancel", serde_json::json!({"task_id":task_id}))
            .await?;
        Ok(())
    }
    pub async fn task_events(
        &self,
        task_id: &str,
        after_sequence: u64,
        wait_ms: u64,
    ) -> Result<Vec<TaskEvent>, String> {
        serde_json::from_value(self.call("task.events", serde_json::json!({"task_id":task_id,"after_sequence":after_sequence,"wait_ms":wait_ms})).await?)
            .map_err(|e| e.to_string())
    }
    pub async fn wait_task(&self, task_id: &str) -> Result<TaskInfo, String> {
        loop {
            let task = self.task(task_id).await?;
            match task.status {
                TaskStatus::Succeeded => return Ok(task),
                TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted => {
                    return Err(task
                        .error
                        .clone()
                        .unwrap_or_else(|| format!("Task ended with status {:?}", task.status)));
                }
                TaskStatus::Queued | TaskStatus::Running => {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await
                }
            }
        }
    }
    pub fn subscribe_task(&self, task_id: impl Into<String>) -> TaskSubscription {
        TaskSubscription {
            launcher: self.clone(),
            task_id: task_id.into(),
            after_sequence: 0,
        }
    }
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let request = umc_api::RpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: method.into(),
            params,
        };
        let response = if self.daemon {
            call_local_daemon(&self.data_dir, request).await?
        } else {
            self.core.dispatch(request).await
        };
        if let Some(error) = response.error {
            return Err(error.message);
        }
        response
            .result
            .ok_or_else(|| "Core returned no result".into())
    }
    pub fn data_dir(&self) -> &std::path::Path {
        self.core.data_dir()
    }
}

pub struct TaskSubscription {
    launcher: Launcher,
    task_id: String,
    after_sequence: u64,
}

impl TaskSubscription {
    /// Long-polls for newly persisted events, returning an empty vector on timeout.
    pub async fn next_events(&mut self) -> Result<Vec<TaskEvent>, String> {
        let events = self
            .launcher
            .task_events(&self.task_id, self.after_sequence, 30_000)
            .await?;
        if let Some(last) = events.last() {
            self.after_sequence = last.sequence;
        }
        Ok(events)
    }
    pub fn after_sequence(&self) -> u64 {
        self.after_sequence
    }
}

impl LauncherBuilder {
    pub fn data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = path.into();
        self
    }
    /// Route calls through the local `umcd` process over the platform IPC endpoint.
    pub fn connect_daemon(mut self) -> Self {
        self.daemon = true;
        self
    }
    /// Register an in-process loader provider. Daemon connections use the
    /// providers configured by the daemon host itself.
    pub fn register_loader(mut self, adapter: Arc<dyn LoaderAdapter>) -> Self {
        self.loaders.register(adapter);
        self
    }
    pub async fn build(self) -> Result<Launcher, std::io::Error> {
        if !self.daemon {
            std::fs::create_dir_all(&self.data_dir)?;
        }
        let core = UmcCore::with_loaders(self.data_dir, self.loaders);
        let data_dir = core.data_dir().to_path_buf();
        Ok(Launcher {
            core,
            data_dir,
            daemon: self.daemon,
        })
    }
}

async fn call_local_daemon(
    data_dir: &std::path::Path,
    request: umc_api::RpcRequest,
) -> Result<umc_api::RpcResponse, String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    #[cfg(unix)]
    let stream = tokio::net::UnixStream::connect(data_dir.join("umc.sock"))
        .await
        .map_err(|e| format!("Could not connect to UMC daemon: {e}"))?;
    #[cfg(windows)]
    let stream = tokio::net::windows::named_pipe::ClientOptions::new()
        .open(r"\\.\pipe\umc")
        .map_err(|e| format!("Could not connect to UMC daemon: {e}"))?;
    #[cfg(not(any(unix, windows)))]
    return Err("Local daemon transport is unsupported on this platform".into());

    let (read, mut write) = tokio::io::split(stream);
    let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write.write_all(&bytes).await.map_err(|e| e.to_string())?;
    write.flush().await.map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(read);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&response).map_err(|e| format!("Invalid response from UMC daemon: {e}"))
}

#[cfg(all(test, unix))]
mod daemon_tests {
    use super::*;

    #[tokio::test]
    async fn daemon_reconnects_and_recovers_stale_process_state() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("umc-daemon-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let process_dir = root.join("processes");
        std::fs::create_dir_all(&process_dir).unwrap();
        let stale = umc_api::GameProcess {
            process_id: "stale-process".into(),
            instance_id: "test-instance".into(),
            pid: u32::MAX - 1,
            kind: umc_api::ProcessKind::Process,
            status: umc_api::ProcessStatus::Running,
            started_at: "2026-01-01T00:00:00Z".into(),
            exit_code: None,
        };
        use std::os::unix::process::CommandExt;
        let live_id = "reconnected-process";
        let marker = format!("-Dumc.process_id={live_id}");
        let mut child = std::process::Command::new("/bin/sleep")
            .arg0(&marker)
            .arg("30")
            .spawn()
            .unwrap();
        let live = umc_api::GameProcess {
            process_id: live_id.into(),
            instance_id: "test-instance".into(),
            pid: child.id(),
            kind: umc_api::ProcessKind::Process,
            status: umc_api::ProcessStatus::Running,
            started_at: "2026-01-02T00:00:00Z".into(),
            exit_code: None,
        };
        std::fs::write(
            process_dir.join("stale-process.json"),
            serde_json::to_vec(&stale).unwrap(),
        )
        .unwrap();
        std::fs::write(
            process_dir.join("reconnected-process.json"),
            serde_json::to_vec(&live).unwrap(),
        )
        .unwrap();

        let daemon = tokio::spawn(umc_core::serve_local_daemon(UmcCore::new(root.clone())));
        let socket = root.join("umc.sock");
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            if daemon.is_finished() {
                panic!("daemon exited before binding: {:?}", daemon.await);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(socket.exists(), "daemon did not bind its local socket");
        assert_eq!(
            std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let launcher = Launcher::builder()
            .data_dir(root.clone())
            .connect_daemon()
            .build()
            .await
            .unwrap();
        let processes = launcher.processes().await.unwrap();
        assert_eq!(processes.len(), 2);
        let recovered_stale = processes
            .iter()
            .find(|p| p.process_id == "stale-process")
            .unwrap();
        assert!(matches!(
            recovered_stale.status,
            umc_api::ProcessStatus::Exited
        ));
        let recovered_live = processes.iter().find(|p| p.process_id == live_id).unwrap();
        assert!(matches!(
            recovered_live.status,
            umc_api::ProcessStatus::Running
        ));
        launcher.stop_process(live_id).await.unwrap();
        for _ in 0..100 {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            child.try_wait().unwrap().is_some(),
            "reconnected process did not stop"
        );
        assert!(matches!(
            launcher.process_status(live_id).await.unwrap().status,
            umc_api::ProcessStatus::Exited
        ));

        daemon.abort();
        let _ = daemon.await;
        let _ = std::fs::remove_dir_all(root);
    }
}
