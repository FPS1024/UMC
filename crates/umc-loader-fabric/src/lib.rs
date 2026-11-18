//! Fabric Meta API adapter. It resolves Fabric profiles; Core performs the
//! shared download and task handling around the resulting strategy.
use reqwest::Client;
use serde::Deserialize;
use umc_loader_api::{
    LoaderAdapter, LoaderId, LoaderInstallPlan, LoaderInstallRequest, LoaderInstallStrategy,
    LoaderVersion,
};
use url::Url;

const META_BASE: &str = "https://meta.fabricmc.net/";

pub struct FabricAdapter {
    client: Client,
}

impl Default for FabricAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FabricAdapter {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("FPS1024/UMC/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("static Fabric HTTP client configuration is valid");
        Self { client }
    }

    fn endpoint(segments: &[&str]) -> Result<Url, String> {
        let mut url = Url::parse(META_BASE).map_err(|e| e.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "Invalid Fabric Meta base URL")?
            .extend(segments.iter().copied());
        Ok(url)
    }
}

#[derive(Deserialize)]
struct FabricEntry {
    loader: FabricLoaderVersion,
}

#[derive(Deserialize)]
struct FabricLoaderVersion {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[async_trait::async_trait]
impl LoaderAdapter for FabricAdapter {
    fn id(&self) -> LoaderId {
        LoaderId::fabric()
    }

    async fn list_versions(&self, minecraft_version: &str) -> Result<Vec<LoaderVersion>, String> {
        validate_version(minecraft_version)?;
        let url = Self::endpoint(&["v2", "versions", "loader", minecraft_version])?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Fabric Meta request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Fabric Meta returned an error: {e}"))?;
        let entries: Vec<FabricEntry> = response
            .json()
            .await
            .map_err(|e| format!("Invalid Fabric Meta response: {e}"))?;
        Ok(entries
            .into_iter()
            .map(|entry| LoaderVersion {
                loader: LoaderId::fabric(),
                minecraft_version: minecraft_version.to_owned(),
                version: entry.loader.version,
                stable: entry.loader.stable,
            })
            .collect())
    }

    async fn plan_install(
        &self,
        request: LoaderInstallRequest,
    ) -> Result<LoaderInstallPlan, String> {
        let versions = self.list_versions(&request.minecraft_version).await?;
        let selected = select_version(&versions, request.loader_version.as_deref())?;
        let url = Self::endpoint(&[
            "v2",
            "versions",
            "loader",
            &request.minecraft_version,
            &selected.version,
            "profile",
            "json",
        ])?;
        Ok(LoaderInstallPlan {
            loader: LoaderId::fabric(),
            minecraft_version: request.minecraft_version.clone(),
            loader_version: selected.version.clone(),
            version_id: format!(
                "fabric-loader-{}-{}",
                selected.version, request.minecraft_version
            ),
            strategy: LoaderInstallStrategy::FabricProfile {
                profile_url: url.to_string(),
            },
        })
    }
}

fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty() || version.len() > 64 || !version.is_ascii() || version.contains('/') {
        return Err("Invalid Minecraft version for Fabric adapter".into());
    }
    Ok(())
}

fn select_version<'a>(
    versions: &'a [LoaderVersion],
    requested: Option<&str>,
) -> Result<&'a LoaderVersion, String> {
    if let Some(version) = requested {
        return versions
            .iter()
            .find(|candidate| candidate.version == version)
            .ok_or_else(|| format!("Fabric Loader version {version} is not available"));
    }
    versions
        .iter()
        .find(|candidate| candidate.stable)
        .or_else(|| versions.first())
        .ok_or_else(|| "No Fabric Loader versions are available for this Minecraft version".into())
}
