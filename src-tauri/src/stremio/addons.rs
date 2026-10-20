//! The user's installed addons (the account's addon collection).
//!
//! Descriptors are kept as raw JSON so that fields Spaceport does not know about survive a
//! round trip when the collection is written back to Stremio.

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct AddonCollection {
    #[serde(default)]
    pub addons: Vec<Value>,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum AddonError {
    #[error("not a valid addon URL")]
    InvalidUrl,
    #[error("addon URLs must start with https://, http:// or stremio://")]
    UnsupportedScheme,
    #[error("addon URLs must end with /manifest.json")]
    NotAManifestUrl,
    #[error("invalid addon manifest: {0}")]
    InvalidManifest(String),
    #[error("addon is not installed")]
    NotInstalled,
    #[error("this addon is protected and cannot be removed")]
    Protected,
    #[error("the new order must contain every installed addon exactly once")]
    BadOrder,
    #[error("this addon has no configure page")]
    NotConfigurable,
}

/// An installed addon as the UI sees it, in the user's order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Addon {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub transport_url: String,
    pub official: bool,
    pub protected: bool,
    pub configurable: bool,
}

fn str_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn bool_field(value: &Value, path: &[&str]) -> bool {
    path.iter()
        .try_fold(value, |v, key| v.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn transport_url(descriptor: &Value) -> Option<&str> {
    descriptor.get("transportUrl").and_then(Value::as_str)
}

impl Addon {
    /// Builds the view of one descriptor. Malformed entries are skipped by returning `None`.
    pub fn from_descriptor(descriptor: &Value) -> Option<Self> {
        let manifest = descriptor.get("manifest")?;
        let id = str_field(manifest, "id")?;
        let name = str_field(manifest, "name")
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| id.clone());
        Some(Self {
            name,
            version: str_field(manifest, "version").unwrap_or_default(),
            description: str_field(manifest, "description"),
            logo: str_field(manifest, "logo"),
            transport_url: transport_url(descriptor)?.to_string(),
            official: bool_field(descriptor, &["flags", "official"]),
            protected: bool_field(descriptor, &["flags", "protected"]),
            configurable: bool_field(manifest, &["behaviorHints", "configurable"]),
            id,
        })
    }
}

pub fn addon_views(collection: &[Value]) -> Vec<Addon> {
    collection
        .iter()
        .filter_map(Addon::from_descriptor)
        .collect()
}

/// Whether an addon declares `resource` for this type and id. A resource is either a plain name,
/// which uses the manifest's `types` and `idPrefixes`, or an object with its own `types` and
/// `idPrefixes`. Without `idPrefixes` any id is accepted.
pub fn supports(manifest: &Value, resource: &str, kind: &str, id: &str) -> bool {
    let list = |v: &Value, key: &str| -> Option<Vec<String>> {
        v.get(key).and_then(Value::as_array).map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
    };
    let Some(resources) = manifest.get("resources").and_then(Value::as_array) else {
        return false;
    };
    resources.iter().any(|r| {
        let (types, prefixes) = match r {
            Value::String(name) if name == resource => {
                (list(manifest, "types"), list(manifest, "idPrefixes"))
            }
            Value::Object(_) if r.get("name").and_then(Value::as_str) == Some(resource) => (
                list(r, "types").or_else(|| list(manifest, "types")),
                list(r, "idPrefixes").or_else(|| list(manifest, "idPrefixes")),
            ),
            _ => return false,
        };
        let type_ok = types.is_some_and(|t| t.iter().any(|t| t == kind));
        let id_ok = match prefixes {
            Some(p) if !p.is_empty() => p.iter().any(|p| id.starts_with(p.as_str())),
            _ => true,
        };
        type_ok && id_ok
    })
}

/// Transport URLs of the addons to ask for `resource`, in collection order, with `preferred`
/// (usually the addon a title came from) first when it supports the request.
pub fn resource_sources(
    collection: &[Value],
    resource: &str,
    kind: &str,
    id: &str,
    preferred: Option<&str>,
) -> Vec<String> {
    let mut urls: Vec<String> = collection
        .iter()
        .filter(|d| {
            d.get("manifest")
                .is_some_and(|m| supports(m, resource, kind, id))
        })
        .filter_map(|d| transport_url(d).map(str::to_string))
        .collect();
    if let Some(index) = preferred.and_then(|p| urls.iter().position(|u| u == p)) {
        let first = urls.remove(index);
        urls.insert(0, first);
    }
    urls
}

/// Turns what the user typed into a manifest URL. `stremio://` links are served over https.
pub fn normalize_manifest_url(input: &str) -> Result<String, AddonError> {
    let input = input.trim();
    let input = match input.strip_prefix("stremio://") {
        Some(rest) => format!("https://{rest}"),
        None => input.to_string(),
    };
    let url = Url::parse(&input).map_err(|_| AddonError::InvalidUrl)?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(AddonError::UnsupportedScheme);
    }
    if url.host_str().is_none() {
        return Err(AddonError::InvalidUrl);
    }
    if !url.path().ends_with("/manifest.json") {
        return Err(AddonError::NotAManifestUrl);
    }
    Ok(url.to_string())
}

/// Checks the fields the addon protocol requires in a manifest.
pub fn validate_manifest(manifest: &Value) -> Result<(), AddonError> {
    let missing = |field: &str| AddonError::InvalidManifest(format!("missing `{field}`"));
    for field in ["id", "name", "version"] {
        if !str_field(manifest, field).is_some_and(|v| !v.trim().is_empty()) {
            return Err(missing(field));
        }
    }
    for field in ["resources", "types"] {
        if !manifest.get(field).is_some_and(Value::is_array) {
            return Err(missing(field));
        }
    }
    if manifest.get("catalogs").is_some_and(|c| !c.is_array()) {
        return Err(AddonError::InvalidManifest(
            "`catalogs` must be a list".to_string(),
        ));
    }
    Ok(())
}

/// Adds an addon to the end of the collection, or refreshes its manifest in place when the
/// same URL is already installed so the user's order and flags are kept.
pub fn install(collection: &mut Vec<Value>, transport_url_value: &str, manifest: Value) {
    if let Some(existing) = collection
        .iter_mut()
        .find(|d| transport_url(d) == Some(transport_url_value))
    {
        existing["manifest"] = manifest;
        return;
    }
    collection.push(json!({
        "transportUrl": transport_url_value,
        "manifest": manifest,
        "flags": { "official": false, "protected": false },
    }));
}

/// A descriptor for an addon Spaceport uses by default without it being in the account.
pub fn builtin_descriptor(transport_url_value: &str, manifest: Value) -> Value {
    json!({
        "transportUrl": transport_url_value,
        "manifest": manifest,
        "flags": { "official": true, "protected": true },
    })
}

pub fn remove(collection: &mut Vec<Value>, url: &str) -> Result<(), AddonError> {
    let index = collection
        .iter()
        .position(|d| transport_url(d) == Some(url))
        .ok_or(AddonError::NotInstalled)?;
    if bool_field(&collection[index], &["flags", "protected"]) {
        return Err(AddonError::Protected);
    }
    collection.remove(index);
    Ok(())
}

/// Puts the collection in the order given by `order`, a list of transport URLs.
pub fn reorder(collection: &mut Vec<Value>, order: &[String]) -> Result<(), AddonError> {
    if order.len() != collection.len() {
        return Err(AddonError::BadOrder);
    }
    let mut remaining = collection.clone();
    let mut sorted = Vec::with_capacity(remaining.len());
    for url in order {
        let index = remaining
            .iter()
            .position(|d| transport_url(d) == Some(url.as_str()))
            .ok_or(AddonError::BadOrder)?;
        sorted.push(remaining.remove(index));
    }
    *collection = sorted;
    Ok(())
}

/// The configure page of a configurable addon: its base URL followed by `/configure`.
pub fn configure_url(collection: &[Value], url: &str) -> Result<String, AddonError> {
    let addon = collection
        .iter()
        .filter_map(Addon::from_descriptor)
        .find(|a| a.transport_url == url)
        .ok_or(AddonError::NotInstalled)?;
    if !addon.configurable {
        return Err(AddonError::NotConfigurable);
    }
    let base = url
        .strip_suffix("/manifest.json")
        .ok_or(AddonError::NotAManifestUrl)?;
    Ok(format!("{base}/configure"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CINEMETA: &str = "https://v3-cinemeta.strem.io/manifest.json";
    const OTHER: &str = "https://example.org/addon/manifest.json";

    fn sample() -> Vec<Value> {
        let collection: AddonCollection = serde_json::from_str(
            r#"{"addons":[
                {"transportUrl":"https://v3-cinemeta.strem.io/manifest.json",
                 "manifest":{"id":"com.linvo.cinemeta","name":"Cinemeta","version":"3.0.13",
                             "resources":["catalog","meta"],"types":["movie","series"],
                             "catalogs":[]},
                 "flags":{"official":true,"protected":true}},
                {"transportUrl":"https://example.org/addon/manifest.json",
                 "manifest":{"id":"org.example.addon","version":"1.0.0",
                             "behaviorHints":{"configurable":true}},
                 "extra":"kept"},
                {"manifest":{"id":"broken"}}
            ],"lastModified":1700000000000}"#,
        )
        .unwrap();
        collection.addons
    }

    fn manifest(id: &str) -> Value {
        json!({ "id": id, "name": "Test", "version": "1.0.0", "resources": ["stream"],
                "types": ["movie"], "catalogs": [] })
    }

    fn urls(collection: &[Value]) -> Vec<&str> {
        collection.iter().filter_map(transport_url).collect()
    }

    #[test]
    fn views_keep_order_and_skip_malformed() {
        let addons = addon_views(&sample());
        assert_eq!(addons.len(), 2);
        assert_eq!(addons[0].name, "Cinemeta");
        assert!(addons[0].official && addons[0].protected && !addons[0].configurable);
        // A manifest without a name falls back to its id; missing flags default to false.
        assert_eq!(addons[1].name, "org.example.addon");
        assert!(!addons[1].official && !addons[1].protected && addons[1].configurable);
    }

    #[test]
    fn normalizes_manifest_urls() {
        assert_eq!(
            normalize_manifest_url("  stremio://example.org/x/manifest.json ").unwrap(),
            "https://example.org/x/manifest.json"
        );
        assert_eq!(
            normalize_manifest_url("http://127.0.0.1:7000/manifest.json").unwrap(),
            "http://127.0.0.1:7000/manifest.json"
        );
        assert_eq!(
            normalize_manifest_url("ftp://example.org/manifest.json"),
            Err(AddonError::UnsupportedScheme)
        );
        assert_eq!(
            normalize_manifest_url("https://example.org/addon"),
            Err(AddonError::NotAManifestUrl)
        );
        assert_eq!(
            normalize_manifest_url("not a url"),
            Err(AddonError::InvalidUrl)
        );
    }

    #[test]
    fn validates_manifests() {
        assert_eq!(validate_manifest(&manifest("a")), Ok(()));
        let mut no_name = manifest("a");
        no_name["name"] = json!("");
        assert!(validate_manifest(&no_name).is_err());
        let mut no_types = manifest("a");
        no_types.as_object_mut().unwrap().remove("types");
        assert!(validate_manifest(&no_types).is_err());
        let mut bad_catalogs = manifest("a");
        bad_catalogs["catalogs"] = json!({});
        assert!(validate_manifest(&bad_catalogs).is_err());
    }

    #[test]
    fn install_appends_new_addon() {
        let mut c = sample();
        install(&mut c, "https://new.example/manifest.json", manifest("new"));
        assert_eq!(c.len(), 4);
        assert_eq!(
            transport_url(&c[3]),
            Some("https://new.example/manifest.json")
        );
        assert_eq!(c[3]["flags"]["protected"], json!(false));
    }

    #[test]
    fn reinstall_updates_in_place() {
        let mut c = sample();
        install(&mut c, OTHER, manifest("org.example.addon"));
        assert_eq!(c.len(), 3);
        assert_eq!(c[1]["manifest"]["name"], json!("Test"));
        assert_eq!(c[1]["extra"], json!("kept"));
    }

    #[test]
    fn remove_respects_protection() {
        let mut c = sample();
        assert_eq!(remove(&mut c, CINEMETA), Err(AddonError::Protected));
        assert_eq!(
            remove(&mut c, "https://nope/manifest.json"),
            Err(AddonError::NotInstalled)
        );
        remove(&mut c, OTHER).unwrap();
        assert_eq!(urls(&c), [CINEMETA]);
    }

    #[test]
    fn reorders_collection() {
        let mut c: Vec<Value> = sample().into_iter().take(2).collect();
        reorder(&mut c, &[OTHER.to_string(), CINEMETA.to_string()]).unwrap();
        assert_eq!(urls(&c), [OTHER, CINEMETA]);
    }

    #[test]
    fn rejects_bad_order_without_changes() {
        let mut c: Vec<Value> = sample().into_iter().take(2).collect();
        assert_eq!(
            reorder(&mut c, &[OTHER.to_string()]),
            Err(AddonError::BadOrder)
        );
        assert_eq!(
            reorder(&mut c, &[OTHER.to_string(), OTHER.to_string()]),
            Err(AddonError::BadOrder)
        );
        assert_eq!(urls(&c), [CINEMETA, OTHER]);
    }

    #[test]
    fn matches_resources_by_type_and_prefix() {
        let simple = json!({"resources": ["meta", "stream"], "types": ["movie", "series"],
                            "idPrefixes": ["tt"]});
        assert!(supports(&simple, "meta", "series", "tt0903747"));
        assert!(!supports(&simple, "meta", "series", "kitsu:1"));
        assert!(!supports(&simple, "meta", "anime", "tt1"));
        assert!(!supports(&simple, "subtitles", "movie", "tt1"));

        let detailed = json!({"resources": [
                {"name": "meta", "types": ["anime"], "idPrefixes": ["kitsu:"]},
                "catalog"], "types": ["anime", "movie"]});
        assert!(supports(&detailed, "meta", "anime", "kitsu:1"));
        assert!(!supports(&detailed, "meta", "movie", "kitsu:1"));

        let open = json!({"resources": [{"name": "meta"}], "types": ["movie"], "idPrefixes": []});
        assert!(supports(&open, "meta", "movie", "anything"));
    }

    #[test]
    fn orders_resource_sources_with_preferred_first() {
        let collection = vec![
            json!({"transportUrl": "a", "manifest": {"resources": ["meta"], "types": ["movie"]}}),
            json!({"transportUrl": "b", "manifest": {"resources": ["stream"], "types": ["movie"]}}),
            json!({"transportUrl": "c", "manifest": {"resources": ["meta"], "types": ["movie"]}}),
        ];
        assert_eq!(
            resource_sources(&collection, "meta", "movie", "tt1", None),
            ["a", "c"]
        );
        assert_eq!(
            resource_sources(&collection, "meta", "movie", "tt1", Some("c")),
            ["c", "a"]
        );
        // A preferred addon that cannot answer is not added.
        assert_eq!(
            resource_sources(&collection, "meta", "movie", "tt1", Some("b")),
            ["a", "c"]
        );
    }

    #[test]
    fn builds_configure_url() {
        let c = sample();
        assert_eq!(
            configure_url(&c, OTHER).unwrap(),
            "https://example.org/addon/configure"
        );
        assert_eq!(
            configure_url(&c, CINEMETA),
            Err(AddonError::NotConfigurable)
        );
    }
}
