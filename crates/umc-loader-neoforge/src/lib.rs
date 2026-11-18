//! NeoForge Maven adapter. NeoForge version components encode the Minecraft
//! line; the mapping follows NeoForged's documented versioning scheme.
use reqwest::Client;
use umc_loader_api::{
    LoaderAdapter, LoaderId, LoaderInstallPlan, LoaderInstallRequest, LoaderInstallStrategy,
    LoaderVersion,
};
use url::Url;

const MAVEN_API_BASE: &str = "https://maven.neoforged.net/";
const VERSIONS_PATH: &[&str] = &[
    "api",
    "maven",
    "versions",
    "releases",
    "net",
    "neoforged",
    "neoforge",
];

pub struct NeoForgeAdapter {
    client: Client,
}

impl Default for NeoForgeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NeoForgeAdapter {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("FPS1024/UMC/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("static NeoForge HTTP client configuration is valid");
        Self { client }
    }

    fn maven_url(segments: &[&str]) -> Result<Url, String> {
        let mut url = Url::parse(MAVEN_API_BASE).map_err(|e| e.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "Invalid NeoForge Maven base URL")?
            .extend(segments.iter().copied());
        Ok(url)
    }

    async fn all_versions(&self) -> Result<Vec<String>, String> {
        let url = Self::maven_url(VERSIONS_PATH)?;
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("NeoForge Maven request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("NeoForge Maven returned an error: {e}"))?
            .json::<Vec<String>>()
            .await
            .map_err(|e| format!("Invalid NeoForge Maven version response: {e}"))
    }
}

#[async_trait::async_trait]
impl LoaderAdapter for NeoForgeAdapter {
    fn id(&self) -> LoaderId {
        LoaderId::neoforge()
    }

    async fn list_versions(&self, minecraft_version: &str) -> Result<Vec<LoaderVersion>, String> {
        let requested = canonical_minecraft_version(minecraft_version)?;
        let mut versions = self
            .all_versions()
            .await?
            .into_iter()
            .filter_map(|version| {
                let mapped = mapped_minecraft_version(&version)?;
                (mapped == requested).then(|| LoaderVersion {
                    loader: LoaderId::neoforge(),
                    minecraft_version: minecraft_version.to_owned(),
                    stable: is_stable(&version),
                    version,
                })
            })
            .collect::<Vec<_>>();
        // Maven's endpoint is newest-first today. Keep that order and do not
        // compare Maven/NeoForge version strings lexicographically ourselves.
        Ok(std::mem::take(&mut versions))
    }

    async fn plan_install(
        &self,
        request: LoaderInstallRequest,
    ) -> Result<LoaderInstallPlan, String> {
        let versions = self.list_versions(&request.minecraft_version).await?;
        let selected = select_version(&versions, request.loader_version.as_deref())?;
        let version = selected.version.clone();
        let artifact = format!("neoforge-{version}-installer.jar");
        let base = ["releases", "net", "neoforged", "neoforge", version.as_str()];
        let mut artifact_path = base.to_vec();
        artifact_path.push(artifact.as_str());
        let mut hash_path = artifact_path.clone();
        hash_path.push("sha1");
        let installer_url = Self::maven_url(&artifact_path)?.to_string();
        let sha1_url = Self::maven_url(&hash_path)?.to_string();
        Ok(LoaderInstallPlan {
            loader: LoaderId::neoforge(),
            minecraft_version: request.minecraft_version,
            loader_version: version.clone(),
            version_id: format!("neoforge-{version}"),
            strategy: LoaderInstallStrategy::NeoForgeInstaller {
                installer_url,
                sha1_url,
            },
        })
    }
}

/// Maps NeoForge's documented `major.api.*` scheme to the Minecraft line.
/// Versions for Minecraft 1.x omitted the leading `1`; modern 26.x versions
/// use the full Minecraft year-based major.
fn mapped_minecraft_version(version: &str) -> Option<String> {
    if version.starts_with("1.") {
        return version.split('-').next().map(str::to_owned);
    }
    let mut parts = version.split(['.', '-']);
    let major = parts.next()?.parse::<u32>().ok()?;
    let api = parts.next()?.parse::<u32>().ok()?;
    if major >= 26 {
        Some(format!("{major}.{api}"))
    } else {
        Some(format!("1.{major}.{api}"))
    }
}

fn canonical_minecraft_version(version: &str) -> Result<String, String> {
    if version.is_empty() || version.len() > 64 || !version.is_ascii() || version.contains('/') {
        return Err("Invalid Minecraft version for NeoForge adapter".into());
    }
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() == 2 && parts[0] == "1" {
        Ok(format!("{version}.0"))
    } else {
        Ok(version.to_owned())
    }
}

fn is_stable(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    !["alpha", "beta", "snapshot", "pre", "rc"]
        .iter()
        .any(|tag| lower.contains(tag))
}

fn select_version<'a>(
    versions: &'a [LoaderVersion],
    requested: Option<&str>,
) -> Result<&'a LoaderVersion, String> {
    if let Some(version) = requested {
        return versions
            .iter()
            .find(|candidate| candidate.version == version)
            .ok_or_else(|| {
                format!("NeoForge version {version} is not available for this Minecraft version")
            });
    }
    versions
        .iter()
        .find(|candidate| candidate.stable)
        .or_else(|| versions.first())
        .ok_or_else(|| "No NeoForge versions are available for this Minecraft version".into())
}
