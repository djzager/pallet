use crate::resource::RawResource;
use std::collections::HashMap;

pub struct MergeResult {
    pub resources: Vec<RawResource>,
    pub warnings: Vec<String>,
}

/// Merge resources from multiple sources using hierarchy and governance rules.
///
/// Algorithm:
/// 1. Process resources in source_index order (ascending = most authoritative first)
/// 2. For each resource, key on (kind, name):
///    - If key not seen → insert
///    - If existing is governed → skip with warning (governed resource locked)
///    - If existing is federated → override (later/more-specific source wins)
pub fn merge_resources(mut all_resources: Vec<RawResource>) -> MergeResult {
    // Sort by source_index (ascending = most authoritative first)
    all_resources.sort_by_key(|r| r.source_index);

    let mut merged: HashMap<(String, String), RawResource> = HashMap::new();
    let mut insertion_order: Vec<(String, String)> = Vec::new();
    let mut warnings = Vec::new();

    for resource in all_resources {
        let key = (resource.kind.to_string(), resource.name.clone());

        if let Some(existing) = merged.get(&key) {
            if existing.governance == "governed" {
                warnings.push(format!(
                    "Cannot override governed {}/{} from '{}' (governed by '{}')",
                    key.0, key.1, resource.source_name, existing.source_name
                ));
            } else {
                // Federated — later (more specific) source overrides
                merged.insert(key, resource);
            }
        } else {
            insertion_order.push(key.clone());
            merged.insert(key, resource);
        }
    }

    // Return in insertion order
    let resources = insertion_order
        .into_iter()
        .filter_map(|key| merged.remove(&key))
        .collect();

    MergeResult {
        resources,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{RawResource, ResourceContent, ResourceKind};

    fn make_resource(
        name: &str,
        kind: ResourceKind,
        source: &str,
        source_index: usize,
        governance: &str,
    ) -> RawResource {
        RawResource {
            name: name.to_string(),
            kind,
            source_name: source.to_string(),
            source_index,
            governance: governance.to_string(),
            content: ResourceContent::SingleFile {
                filename: format!("{name}.md"),
                content: Vec::new(),
            },
            globs: None,
            description: None,
        }
    }

    #[test]
    fn test_no_conflict() {
        let resources = vec![
            make_resource("a", ResourceKind::Rule, "src0", 0, "federated"),
            make_resource("b", ResourceKind::Rule, "src1", 1, "federated"),
        ];
        let result = merge_resources(resources);
        assert_eq!(result.resources.len(), 2);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_federated_override() {
        let resources = vec![
            make_resource(
                "coding-standards",
                ResourceKind::Rule,
                "org",
                0,
                "federated",
            ),
            make_resource(
                "coding-standards",
                ResourceKind::Rule,
                "team",
                1,
                "federated",
            ),
        ];
        let result = merge_resources(resources);
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].source_name, "team"); // later source wins
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_governed_cannot_be_overridden() {
        let resources = vec![
            make_resource("security", ResourceKind::Rule, "org", 0, "governed"),
            make_resource("security", ResourceKind::Rule, "team", 1, "federated"),
        ];
        let result = merge_resources(resources);
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].source_name, "org"); // governed wins
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("governed"));
    }

    #[test]
    fn test_different_kinds_no_conflict() {
        let resources = vec![
            make_resource("testing", ResourceKind::Rule, "src0", 0, "federated"),
            make_resource("testing", ResourceKind::Skill, "src0", 0, "federated"),
        ];
        let result = merge_resources(resources);
        assert_eq!(result.resources.len(), 2); // different kinds = no conflict
        assert!(result.warnings.is_empty());
    }
}
