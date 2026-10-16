//! Curated, revision-pinned local model catalog.
use crate::runner::ModelRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelTier {
    Compact,
    Balanced,
    Large,
}
impl ModelTier {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "COMPACT",
            Self::Balanced => "BALANCED",
            Self::Large => "LARGE",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Stable model identity used by preferences and marker records.
    pub name: &'static str,
    pub display_name: &'static str,
    pub family: &'static str,
    pub tier: ModelTier,
    pub parameter_label: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    pub license: &'static str,
    pub source: &'static str,
    pub url: &'static str,
}

impl CatalogEntry {
    pub(crate) fn matches(&self, record: &ModelRecord) -> bool {
        self.name == record.name
            && self.sha256 == record.sha256
            && self.size == record.size
            && self.license == record.license
            && self.source == record.source
    }

    pub fn record(&self) -> ModelRecord {
        ModelRecord {
            name: self.name.into(),
            sha256: self.sha256.into(),
            size: self.size,
            license: self.license.into(),
            source: self.source.into(),
        }
    }
}

/// Curated, revision-pinned Apache-2.0 GGUF models. Default first.
pub const DEFAULT_MODEL: &str = "qwen3-14b:q4_k_m";
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        name: DEFAULT_MODEL,
        display_name: "Qwen3 14B",
        family: "Qwen",
        tier: ModelTier::Large,
        parameter_label: "14B",
        sha256: "500a8806e85ee9c83f3ae08420295592451379b4f8cf2d0f41c15dffeb6b81f0",
        size: 9_001_752_960,
        license: "apache-2.0",
        source: "https://huggingface.co/Qwen/Qwen3-14B-GGUF",
        url: "https://huggingface.co/Qwen/Qwen3-14B-GGUF/resolve/530227a7d994db8eca5ab5ced2fb692b614357fd/Qwen3-14B-Q4_K_M.gguf",
    },
    CatalogEntry {
        name: "qwen2.5-1.5b-instruct:q4_k_m",
        display_name: "Qwen2.5 1.5B Instruct",
        family: "Qwen",
        tier: ModelTier::Compact,
        parameter_label: "1.5B",
        sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
        size: 1_117_320_736,
        license: "apache-2.0",
        source: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF",
        url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/91cad51170dc346986eccefdc2dd33a9da36ead9/qwen2.5-1.5b-instruct-q4_k_m.gguf",
    },
    CatalogEntry {
        name: "qwen3-4b:q4_k_m",
        display_name: "Qwen3 4B",
        family: "Qwen",
        tier: ModelTier::Compact,
        parameter_label: "4B",
        sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        size: 2_497_280_256,
        license: "apache-2.0",
        source: "https://huggingface.co/Qwen/Qwen3-4B-GGUF",
        url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf",
    },
    CatalogEntry {
        name: "smollm2-1.7b-instruct:q4_k_m",
        display_name: "SmolLM2 1.7B Instruct",
        family: "SmolLM",
        tier: ModelTier::Compact,
        parameter_label: "1.7B",
        sha256: "decd2598bc2c8ed08c19adc3c8fdd461ee19ed5708679d1c54ef54a5a30d4f33",
        size: 1_055_609_536,
        license: "apache-2.0",
        source: "https://huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct-GGUF",
        url: "https://huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct-GGUF/resolve/2d4a76a30b4af41ecd395c35725ac11688d4cfe4/smollm2-1.7b-instruct-q4_k_m.gguf",
    },
    CatalogEntry {
        name: "mistral-7b-instruct-v0.3:q4_k_m",
        display_name: "Mistral 7B Instruct v0.3",
        family: "Mistral",
        tier: ModelTier::Balanced,
        parameter_label: "7B",
        sha256: "1270d22c0fbb3d092fb725d4d96c457b7b687a5f5a715abe1e818da303e562b6",
        size: 4_372_812_000,
        license: "apache-2.0",
        source: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF",
        url: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/61fd4167fff3ab01ee1cfe0da183fa27a944db48/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
    },
    CatalogEntry {
        name: "granite-3.3-8b-instruct:q4_k_m",
        display_name: "Granite 3.3 8B Instruct",
        family: "Granite",
        tier: ModelTier::Balanced,
        parameter_label: "8B",
        sha256: "77bcee066a76dcdd10d0d123c87e32c8ec2c74e31b6ffd87ebee49c9ac215dca",
        size: 4_942_873_344,
        license: "apache-2.0",
        source: "https://huggingface.co/ibm-granite/granite-3.3-8b-instruct-GGUF",
        url: "https://huggingface.co/ibm-granite/granite-3.3-8b-instruct-GGUF/resolve/e40e9dd739c7be00fa965c16ce167088190ce114/granite-3.3-8b-instruct-Q4_K_M.gguf",
    },
    CatalogEntry {
        name: "mistral-nemo-instruct-2407:q4_k_m",
        display_name: "Mistral Nemo 12B Instruct",
        family: "Mistral",
        tier: ModelTier::Large,
        parameter_label: "12B",
        sha256: "7c1a10d202d8788dbe5628dc962254d10654c853cae6aaeca0618f05490d4a46",
        size: 7_477_208_192,
        license: "apache-2.0",
        source: "https://huggingface.co/bartowski/Mistral-Nemo-Instruct-2407-GGUF",
        url: "https://huggingface.co/bartowski/Mistral-Nemo-Instruct-2407-GGUF/resolve/a2dd64a0a76ea1bdb2bb6ab6fa5496b003c7c908/Mistral-Nemo-Instruct-2407-Q4_K_M.gguf",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::marker_bytes;

    fn valid_sha(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    }

    #[test]
    fn catalog_is_curated_pinned_and_default_first() {
        assert_eq!(CATALOG.len(), 7);
        assert_eq!(CATALOG[0].name, DEFAULT_MODEL);
        assert_eq!(CATALOG[1].name, "qwen2.5-1.5b-instruct:q4_k_m");
        assert_eq!(CATALOG[2].name, "qwen3-4b:q4_k_m");
        assert_eq!(CATALOG[3].name, "smollm2-1.7b-instruct:q4_k_m");
        assert_eq!(CATALOG[4].name, "mistral-7b-instruct-v0.3:q4_k_m");
        assert_eq!(CATALOG[5].name, "granite-3.3-8b-instruct:q4_k_m");
        assert_eq!(CATALOG[6].name, "mistral-nemo-instruct-2407:q4_k_m");
        for entry in CATALOG {
            assert!(!entry.display_name.is_empty());
            assert!(!entry.family.is_empty());
            assert!(!entry.parameter_label.is_empty());
            assert!(valid_sha(entry.sha256));
            assert_eq!(entry.license, "apache-2.0");
            assert!(entry.url.starts_with(&format!("{}/resolve/", entry.source)));
            assert!(entry.source.starts_with("https://huggingface.co/"));
        }
        assert_eq!(
            String::from_utf8(marker_bytes(&CATALOG[2].record())).unwrap(),
            "{\"file\":\"7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5.gguf\",\"license\":\"apache-2.0\",\"name\":\"qwen3-4b:q4_k_m\",\"schema_version\":1,\"sha256\":\"7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5\",\"size\":2497280256,\"source\":\"https://huggingface.co/Qwen/Qwen3-4B-GGUF\"}\n"
        );
    }
}
