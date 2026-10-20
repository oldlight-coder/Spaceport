//! The user's installed addons (the account's addon collection).

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AddonCollection {
    #[serde(default)]
    pub addons: Vec<RawAddon>,
}

#[derive(Debug, Deserialize)]
pub struct RawAddon {
    #[serde(rename = "transportUrl")]
    pub transport_url: String,
    pub manifest: RawManifest,
    #[serde(default)]
    pub flags: Flags,
}

#[derive(Debug, Deserialize)]
pub struct RawManifest {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Flags {
    #[serde(default)]
    pub official: bool,
    /// Protected addons (such as Cinemeta) cannot be removed by the user.
    #[serde(default)]
    pub protected: bool,
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
}

impl From<RawAddon> for Addon {
    fn from(raw: RawAddon) -> Self {
        let name = if raw.manifest.name.trim().is_empty() {
            raw.manifest.id.clone()
        } else {
            raw.manifest.name
        };
        Self {
            id: raw.manifest.id,
            name,
            version: raw.manifest.version,
            description: raw.manifest.description,
            logo: raw.manifest.logo,
            transport_url: raw.transport_url,
            official: raw.flags.official,
            protected: raw.flags.protected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_collection_in_order() {
        let collection: AddonCollection = serde_json::from_str(
            r#"{"addons":[
                {"transportUrl":"https://v3-cinemeta.strem.io/manifest.json",
                 "manifest":{"id":"com.linvo.cinemeta","name":"Cinemeta","version":"3.0.13",
                             "resources":["catalog","meta"],"types":["movie","series"],
                             "catalogs":[]},
                 "flags":{"official":true,"protected":true}},
                {"transportUrl":"https://example.org/addon/manifest.json",
                 "manifest":{"id":"org.example.addon","version":"1.0.0"}}
            ],"lastModified":1700000000000}"#,
        )
        .unwrap();
        let addons: Vec<Addon> = collection.addons.into_iter().map(Addon::from).collect();
        assert_eq!(addons.len(), 2);
        assert_eq!(addons[0].name, "Cinemeta");
        assert!(addons[0].official && addons[0].protected);
        // A manifest without a name falls back to its id; missing flags default to false.
        assert_eq!(addons[1].name, "org.example.addon");
        assert!(!addons[1].official && !addons[1].protected);
    }
}
