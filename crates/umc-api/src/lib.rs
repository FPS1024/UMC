//! Stable, transport-independent public data contracts for UMC.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RPC_VERSION: &str = "2.0";
pub const UMC_API_VERSION: &str = "v1";
pub const INSTANCE_CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub release_time: Option<String>,
}

/// Memory values are always MiB, independent of UI display conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub minimum_mib: u32,
    pub maximum_mib: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoaderSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceConfig {
    #[serde(default)]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<LoaderSpec>,
    #[serde(default = "default_auto")]
    pub java_runtime: String,
    pub memory: MemoryConfig,
    /// Stored relative to the UMC data directory; adapters must not pass unchecked paths.
    pub game_directory: String,
}

fn default_auto() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub instance_id: String,
    pub username: String,
    /// This scaffold only implements offline mode. Online authentication is not implied.
    pub offline_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub extra_jvm_args: Vec<String>,
    #[serde(default)]
    pub extra_game_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProcess {
    pub process_id: String,
    pub instance_id: String,
    pub pid: u32,
    pub status: ProcessStatus,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Starting,
    Running,
    Exited,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHandle {
    pub task_id: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub task_id: String,
    pub operation: String,
    pub status: TaskStatus,
    pub created_at: String,
    pub updated_at: String,
    pub timeout_seconds: u64,
    pub sequence: u64,
    pub progress: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvent {
    pub task_id: String,
    pub sequence: u64,
    pub timestamp: String,
    pub event: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub memory: Option<MemoryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairInstanceResult {
    pub config: InstanceConfig,
    pub recovered_from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRequest {
    pub instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntimeInfo {
    pub path: String,
    pub major_version: u32,
    pub vendor_version: String,
    #[serde(default)]
    pub compatible: bool,
    #[serde(default)]
    pub required_major: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntimeInstallRequest {
    pub minecraft_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntimeInstallResult {
    pub runtime_id: String,
    pub minecraft_version: String,
    pub java: JavaRuntimeInfo,
}

/// Stable protocol category; concrete UMC server codes are mapped separately.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Internal,
    InvalidParameters,
    NotFound,
    PermissionDenied,
    Network,
    Integrity,
    Authentication,
    JavaRuntime,
    Instance,
    TaskNotFound,
    Unsupported,
}
