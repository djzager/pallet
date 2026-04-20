use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Skill,
    Rule,
    Profile,
    Agent,
    Prompt,
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceKind::Skill => write!(f, "skill"),
            ResourceKind::Rule => write!(f, "rule"),
            ResourceKind::Profile => write!(f, "profile"),
            ResourceKind::Agent => write!(f, "agent"),
            ResourceKind::Prompt => write!(f, "prompt"),
        }
    }
}

impl ResourceKind {
    /// Parse a resource kind from a string (e.g., from frontmatter `type` field)
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "skill" => Some(ResourceKind::Skill),
            "rule" => Some(ResourceKind::Rule),
            "profile" => Some(ResourceKind::Profile),
            "agent" => Some(ResourceKind::Agent),
            "prompt" => Some(ResourceKind::Prompt),
            _ => None,
        }
    }

    /// Return the directory name used for canonical storage
    pub fn dir_name(&self) -> &str {
        match self {
            ResourceKind::Skill => "skills",
            ResourceKind::Rule => "rules",
            ResourceKind::Profile => "profiles",
            ResourceKind::Agent => "agents",
            ResourceKind::Prompt => "prompts",
        }
    }
}

/// A resource discovered from a source, before storage
#[derive(Debug, Clone)]
pub struct RawResource {
    pub name: String,
    pub kind: ResourceKind,
    pub source_name: String,
    pub source_index: usize,
    pub governance: String,
    /// For skills: directory of files; for rules: single file content
    pub content: ResourceContent,
}

#[derive(Debug, Clone)]
pub enum ResourceContent {
    /// A single file (rules, agent prompts, profile yaml)
    SingleFile {
        filename: String,
        content: Vec<u8>,
    },
    /// A directory of files (skills with SKILL.md and supporting files)
    Directory {
        files: Vec<(String, Vec<u8>)>, // (relative_path, content)
    },
    /// Profile bundle (placeholder — profiles are not placed for Claude Code)
    ProfileBundle,
}

/// Metadata parsed from YAML frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub resource_type: Option<String>,
    #[serde(default = "default_governance")]
    pub governance: String,
}

fn default_governance() -> String {
    "federated".to_string()
}

/// Parse YAML frontmatter from markdown content.
/// Frontmatter is delimited by `---` at the start of the file.
pub fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }

    let rest = &content[3..];
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];

    serde_yaml::from_str(yaml).ok()
}

