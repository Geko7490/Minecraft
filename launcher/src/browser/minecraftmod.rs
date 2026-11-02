//! The user's independent, GitHub Releases-backed Horizon .hmod catalog.
use super::{download, FileRef, Kind, Project, Query, Source, SourceId, Version, GAME};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CATALOG_URL: &str =
    "https://raw.githubusercontent.com/Geko7490/MinecraftMod/main/catalog/mods.json";
const RELEASE_PREFIX: &str =
    "https://github.com/Geko7490/MinecraftMod/releases/download/";
const MAX_CATALOG: u64 = 8 << 20;
const MAX_PACKAGE: u64 = 64 << 20;
const FRESH: Duration = Duration::from_secs(600);

pub struct MinecraftMod;

#[derive(Clone, Debug, Deserialize)]
struct Catalog {
    schema: u32,
    generated: String,
    #[serde(default)]
    mods: Vec<ModEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct ModEntry {
    id: String,
    name: String,
    description: String,
    developer: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    icon: Option<String>,
    source: String,
    versions: Vec<ModVersion>,
}

#[derive(Clone, Debug, Deserialize)]
struct ModVersion {
    version: String,
    game_versions: Vec<String>,
    download_url: String,
    file_name: String,
    size: u64,
    sha256: String,
    published: String,
}

static CACHE: Mutex<Option<(Instant, Arc<Catalog>)>> = Mutex::new(None);

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_release_url(url: &str) -> bool {
    url.starts_with(RELEASE_PREFIX) && download::github_release_asset(url)
}

fn parse_catalog(text: &str) -> Result<Catalog, String> {
    let catalog: Catalog = serde_json::from_str(text)
        .map_err(|e| format!("MinecraftMod catalog is invalid JSON: {e}"))?;
    if catalog.schema != 1 {
        return Err(format!("Unsupported MinecraftMod catalog schema: {}", catalog.schema));
    }
    if catalog.generated.trim().is_empty() {
        return Err("MinecraftMod catalog has no generation timestamp".into());
    }
    // Reject duplicate IDs and malformed entries instead of silently offering an unverified file.
    let mut ids = std::collections::BTreeSet::new();
    for item in &catalog.mods {
        if !valid_id(&item.id) || !ids.insert(item.id.as_str()) {
            return Err(format!("Invalid or duplicate mod ID: {:?}", item.id));
        }
        if item.kind != "hmod" || item.name.trim().is_empty() || item.developer.trim().is_empty() {
            return Err(format!("{} has invalid metadata or is not an .hmod", item.id));
        }
        if !item.source.starts_with("https://github.com/Geko7490/MinecraftMod/") {
            return Err(format!("{} has an untrusted source URL", item.id));
        }
        if item.icon.as_deref().is_some_and(|url| !url.starts_with("https://")) {
            return Err(format!("{} has a non-HTTPS icon URL", item.id));
        }
        if item.versions.is_empty() {
            return Err(format!("{} has no published versions", item.id));
        }
        for version in &item.versions {
            if version.version.trim().is_empty()
                || version.game_versions.is_empty()
                || version.published.trim().is_empty()
                || !valid_release_url(&version.download_url)
                || !version.download_url.ends_with(&format!("/{}", version.file_name))
                || super::safe_name(&version.file_name).is_none()
                || !version.file_name.ends_with(".hmod")
                || version.size == 0
                || version.size > MAX_PACKAGE
                || !valid_hash(&version.sha256)
            {
                return Err(format!("{} {} has invalid release metadata", item.id, version.version));
            }
        }
    }
    Ok(catalog)
}

fn catalog() -> Result<Arc<Catalog>, String> {
    if let Some((at, cached)) = CACHE.lock().map_err(|_| "MinecraftMod cache lock failed")?.as_ref() {
        if at.elapsed() < FRESH {
            return Ok(cached.clone());
        }
    }

    let fetched = (|| {
        let response = download::agent(CATALOG_URL)
            .get(CATALOG_URL)
            .call()
            .map_err(|e| format!("MinecraftMod catalog request failed: {e}"))?;
        let mut bytes = Vec::new();
        use std::io::Read;
        response
            .into_reader()
            .take(MAX_CATALOG + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Could not read MinecraftMod catalog: {e}"))?;
        if bytes.len() as u64 > MAX_CATALOG {
            return Err("MinecraftMod catalog exceeds 8 MiB".into());
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| "MinecraftMod catalog is not UTF-8")?;
        parse_catalog(text).map(Arc::new)
    })();

    match fetched {
        Ok(value) => {
            *CACHE.lock().map_err(|_| "MinecraftMod cache lock failed")? =
                Some((Instant::now(), value.clone()));
            Ok(value)
        }
        Err(error) => {
            // An already validated catalog remains usable offline until a fresh fetch succeeds.
            if let Some((_, stale)) = CACHE.lock().map_err(|_| "MinecraftMod cache lock failed")?.as_ref() {
                Ok(stale.clone())
            } else {
                Err(error)
            }
        }
    }
}

fn project(item: &ModEntry) -> Project {
    let mut game_versions: Vec<String> = item.versions.iter()
        .flat_map(|v| v.game_versions.iter().cloned())
        .collect();
    game_versions.sort();
    game_versions.dedup();
    Project {
        source: SourceId::MinecraftMod,
        id: item.id.clone(),
        slug: item.id.clone(),
        title: item.name.clone(),
        summary: item.description.clone(),
        author: item.developer.clone(),
        // The catalog currently has no trustworthy download-count field.
        downloads: 0,
        page: item.source.clone(),
        icon: item.icon.clone(),
        license: "Horizon .hmod".into(),
        kind: Kind::Mod,
        game_versions,
        loaders: vec!["horizon".into()],
    }
}

fn version(item: &ModVersion) -> Version {
    Version {
        id: item.version.clone(),
        number: item.version.clone(),
        published: item.published.clone(),
        game_versions: item.game_versions.clone(),
        loaders: vec!["horizon".into()],
        files: vec![FileRef {
            url: item.download_url.clone(),
            name: item.file_name.clone(),
            size: item.size,
            sha512: None,
            sha256: Some(item.sha256.to_ascii_lowercase()),
            sha1: None,
            primary: true,
        }],
        required: Vec::new(),
        blocked: None,
    }
}

impl Source for MinecraftMod {
    fn id(&self) -> SourceId {
        SourceId::MinecraftMod
    }

    fn search(&self, query: &Query) -> Result<Vec<Project>, String> {
        if query.kind != Kind::Mod {
            return Ok(Vec::new());
        }
        let catalog = catalog()?;
        let needle = query.text.to_lowercase();
        let mut found: Vec<Project> = catalog.mods.iter()
            .filter(|item| {
                query.everything || item.versions.iter().any(|v| v.game_versions.iter().any(|g| g == GAME))
            })
            .filter(|item| needle.is_empty()
                || item.id.to_lowercase().contains(&needle)
                || item.name.to_lowercase().contains(&needle)
                || item.description.to_lowercase().contains(&needle)
                || item.developer.to_lowercase().contains(&needle))
            .map(project)
            .collect();
        found.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        let start = query.offset.min(found.len());
        let end = start.saturating_add(query.limit.clamp(1, 100)).min(found.len());
        Ok(found[start..end].to_vec())
    }

    fn project(&self, id: &str) -> Result<Project, String> {
        let catalog = catalog()?;
        catalog.mods.iter().find(|item| item.id == id)
            .map(project)
            .ok_or_else(|| format!("MinecraftMod: no mod named {id}"))
    }

    fn versions(&self, project: &Project) -> Result<Vec<Version>, String> {
        if project.source != SourceId::MinecraftMod {
            return Err("Expected a MinecraftMod project".into());
        }
        let catalog = catalog()?;
        let item = catalog.mods.iter().find(|item| item.id == project.id)
            .ok_or_else(|| format!("MinecraftMod: no mod named {}", project.id))?;
        let mut versions: Vec<_> = item.versions.iter().map(version).collect();
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        serde_json::json!({
            "schema": 1,
            "generated": "2026-11-02T00:00:00Z",
            "mods": [{
                "id": "example_mod",
                "name": "Example Mod",
                "description": "A test content mod",
                "developer": "Test Author",
                "type": "hmod",
                "icon": null,
                "source": "https://github.com/Geko7490/MinecraftMod/tree/main/mods/example_mod",
                "versions": [{
                    "version": "1.0.0",
                    "game_versions": [GAME],
                    "download_url": "https://github.com/Geko7490/MinecraftMod/releases/download/v1.0.0/example_mod-1.0.0.hmod",
                    "file_name": "example_mod-1.0.0.hmod",
                    "size": 128,
                    "sha256": "a".repeat(64),
                    "published": "2026-11-02T00:00:00Z",
                    "changelog": "Initial"
                }]
            }]
        }).to_string()
    }

    #[test]
    fn parses_only_verified_hmod_release_metadata() {
        let catalog = parse_catalog(&fixture()).unwrap();
        assert_eq!(catalog.mods.len(), 1);
        let p = project(&catalog.mods[0]);
        assert_eq!(p.source, SourceId::MinecraftMod);
        assert_eq!(p.support(), super::super::Support::Works);
        let v = version(&catalog.mods[0].versions[0]);
        assert_eq!(v.file().unwrap().checksum().unwrap().0, "SHA-256");
        assert_eq!(v.file().unwrap().size, 128);
    }

    #[test]
    fn rejects_untrusted_or_unverifiable_assets() {
        let bad_url = fixture().replace(
            "https://github.com/Geko7490/MinecraftMod/releases/download/",
            "https://evil.example/releases/download/"
        );
        assert!(parse_catalog(&bad_url).is_err());
        let bad_hash = fixture().replace(&"a".repeat(64), "not-a-sha256");
        assert!(parse_catalog(&bad_hash).is_err());
        let bad_type = fixture().replace("\"hmod\"", "\"jar\"");
        assert!(parse_catalog(&bad_type).is_err());
    }
}
