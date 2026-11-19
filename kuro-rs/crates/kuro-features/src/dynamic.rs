//! Run-time discovered features (n8n, hub tools, MCP). Stub until phase 10+.

use crate::manifest::FeatureManifest;

pub trait DynamicSource: Send + Sync {
    fn list_manifests(&self) -> Vec<FeatureManifest>;
}
