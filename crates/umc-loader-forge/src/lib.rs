//! Forge adapter for Maven version metadata and installer artifacts.
use quick_xml::{Reader, events::Event};
use reqwest::Client;
use umc_loader_api::{
    LoaderAdapter, LoaderId, LoaderInstallPlan, LoaderInstallRequest, LoaderInstallStrategy,
    LoaderVersion,
};
use url::Url;

const MAVEN_BASE: &str = "https://maven.minecraftforge.net/";
const METADATA_PATH: &[&str] = &["net", "minecraftforge", "forge", "maven-metadata.xml"];

pub struct ForgeAdapter {
    client: Client,
}

impl Default for ForgeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeAdapter {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("FPS1024/UMC/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("static Forge HTTP client configuration is valid");
        Self { client }
    }

    fn maven_url(segments: &[&str]) -> Result<Url, String> {
        let mut url = Url::parse(MAVEN_BASE).map_err(|e| e.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "Invalid Forge Maven base URL")?
            .extend(segments.iter().copied());
        Ok(url)
    }

    async fn maven_versions(&self) -> Result<Vec<String>, String> {
        let url = Self::maven_url(METADATA_PATH)?;
        let xml = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Forge Maven request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Forge Maven returned an error: {e}"))?
            .text()
            .await
            .map_err(|e| format!("Could not read Forge Maven metadata: {e}"))?;
        parse_maven_versions(&xml)
    }
}

#[async_trait::async_trait]
impl LoaderAdapter for ForgeAdapter {
    fn id(&self) -> LoaderId {
        LoaderId::forge()
    }

    async fn list_versions(&self, minecraft_version: &str) -> Result<Vec<LoaderVersion>, String> {
        validate_version(minecraft_version)?;
        let prefix = format!("{minecraft_version}-");
        let mut versions = self
            .maven_versions()
            .await?
            .into_iter()
            .filter_map(|maven_version| {
                maven_version
                    .strip_prefix(&prefix)
                    .filter(|version| !version.is_empty())
                    .map(|version| LoaderVersion {
                        loader: LoaderId::forge(),
                        minecraft_version: minecraft_version.to_owned(),
                        version: version.to_owned(),
                        stable: is_stable(version),
                    })
            })
            .collect::<Vec<_>>();
        versions.reverse();
        Ok(versions)
    }

    async fn plan_install(
        &self,
        request: LoaderInstallRequest,
    ) -> Result<LoaderInstallPlan, String> {
        let versions = self.list_versions(&request.minecraft_version).await?;
        let selected = select_version(&versions, request.loader_version.as_deref())?;
        let maven_version = format!("{}-{}", request.minecraft_version, selected.version);
        let artifact = format!("forge-{maven_version}-installer.jar");
        let base = ["net", "minecraftforge", "forge", maven_version.as_str()];
        let mut artifact_path = base.to_vec();
        artifact_path.push(artifact.as_str());
        let mut hash_path = artifact_path.clone();
        hash_path.push("sha1");
        let installer_url = Self::maven_url(&artifact_path)?.to_string();
        let sha1_url = Self::maven_url(&hash_path)?.to_string();
        Ok(LoaderInstallPlan {
            loader: LoaderId::forge(),
            minecraft_version: request.minecraft_version,
            loader_version: selected.version.clone(),
            version_id: format!("forge-{maven_version}"),
            strategy: LoaderInstallStrategy::ForgeInstaller {
                installer_url,
                sha1_url,
            },
        })
    }
}

fn parse_maven_versions(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_version = false;
    let mut versions = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if element.name().as_ref() == b"version" => {
                in_version = true;
            }
            Ok(Event::Text(text)) if in_version => {
                let value = text
                    .decode()
                    .map_err(|e| format!("Invalid Forge Maven XML: {e}"))?;
                if !value.trim().is_empty() {
                    versions.push(value.into_owned());
                }
            }
            Ok(Event::End(element)) if element.name().as_ref() == b"version" => {
                in_version = false;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("Invalid Forge Maven XML: {error}")),
        }
    }
    if versions.is_empty() {
        return Err("Forge Maven metadata contains no versions".into());
    }
    Ok(versions)
}

fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty() || version.len() > 64 || !version.is_ascii() || version.contains('/') {
        return Err("Invalid Minecraft version for Forge adapter".into());
    }
    Ok(())
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
                format!("Forge version {version} is not available for this Minecraft version")
            });
    }
    versions
        .iter()
        .find(|candidate| candidate.stable)
        .or_else(|| versions.first())
        .ok_or_else(|| "No Forge versions are available for this Minecraft version".into())
}
