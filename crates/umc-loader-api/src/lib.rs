//! Public, transport-independent contracts for Minecraft loader adapters.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct LoaderId(String);

impl LoaderId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.".contains(&byte)
            })
        {
            return Err(
                "loader id must be 1-64 lowercase ASCII letters, digits, '-', '_' or '.'".into(),
            );
        }
        Ok(Self(value))
    }

    pub fn fabric() -> Self {
        Self("fabric".into())
    }
    pub fn forge() -> Self {
        Self("forge".into())
    }
    pub fn neoforge() -> Self {
        Self("neoforge".into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderVersion {
    pub loader: LoaderId,
    pub minecraft_version: String,
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderInstallRequest {
    pub minecraft_version: String,
    /// `None` selects the adapter's newest stable compatible version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
}

/// A loader-specific preflight result. It deliberately preserves the installer
/// mechanism instead of pretending every loader has the same installation flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum LoaderInstallStrategy {
    FabricProfile {
        profile_url: String,
    },
    ForgeInstaller {
        installer_url: String,
        sha1_url: String,
    },
    NeoForgeInstaller {
        installer_url: String,
        sha1_url: String,
    },
    Custom {
        provider: String,
        payload: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderInstallPlan {
    pub loader: LoaderId,
    pub minecraft_version: String,
    pub loader_version: String,
    pub version_id: String,
    pub strategy: LoaderInstallStrategy,
}

/// Common discovery and preflight API. Installation execution is performed by
/// the Core task engine so download verification, cancellation and persistence
/// remain shared while each strategy keeps its own semantics.
#[async_trait]
pub trait LoaderAdapter: Send + Sync {
    fn id(&self) -> LoaderId;

    async fn list_versions(&self, minecraft_version: &str) -> Result<Vec<LoaderVersion>, String>;

    async fn plan_install(
        &self,
        request: LoaderInstallRequest,
    ) -> Result<LoaderInstallPlan, String>;
}
