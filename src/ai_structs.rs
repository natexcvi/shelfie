use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct FileNameAnalysis {
    pub is_indicative: bool,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SuggestedFileName {
    pub new_name: String,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct FileInfo {
    pub path: String,
    pub suggested_name: Option<String>,
    pub file_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct DirectoryStructure {
    pub directories: Vec<String>,
    pub file_placements: Vec<FilePlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct FilePlacement {
    pub original_path: String,
    pub new_directory: String,
    pub new_name: String,
    pub reasoning: String,
}