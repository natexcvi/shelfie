use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ProcessingItem {
    File(EnrichedFile),
    Directory(EnrichedDirectory),
}

#[derive(Debug, Clone)]
pub struct EnrichedFile {
    pub path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub file_type: String,
    pub size: u64,
    pub content_preview: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnrichedDirectory {
    pub path: PathBuf,
    pub name: String,
    pub sampled_items: Vec<SampledItem>,
}

#[derive(Debug, Clone)]
pub struct SampledItem {
    pub name: String,
    pub is_file: bool,
    pub extension: Option<String>,
}

// LLM extraction structures
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BatchAnalysisRequest {
    pub items: Vec<ItemMetadata>,
    pub existing_cabinets: Vec<CabinetInfo>,
    pub existing_shelves: Vec<ShelfInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ItemMetadata {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub extension: String,             // Use empty string if no extension
    pub size_bytes: u64,               // Use 0 for directories or unknown
    pub sampled_contents: Vec<String>, // Use empty vec for files
    pub content_preview: String,       // Use empty string if no preview
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CabinetInfo {
    pub id: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ShelfInfo {
    pub id: i64,
    pub cabinet_id: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BatchAnalysisResponse {
    pub items: Vec<ItemAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ItemAnalysis {
    pub id: String,
    pub description: String,
    pub suggested_name: String, // Use empty string if no suggestion
    pub is_opaque_directory: bool,
    pub cabinet: CabinetAssignment,
    pub shelf: ShelfAssignment,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CabinetAssignment {
    pub assignment_type: String, // "existing" or "new"
    pub existing_id: i64,        // Use 0 for new assignments
    pub new_name: String,        // Use empty string for existing assignments
    pub new_description: String, // Use empty string for existing assignments
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ShelfAssignment {
    pub assignment_type: String, // "existing" or "new"
    pub existing_id: i64,        // Use 0 for new assignments
    pub new_name: String,        // Use empty string for existing assignments
    pub new_description: String, // Use empty string for existing assignments
}

// Organization preview structures
#[derive(Debug, Clone)]
pub struct OrganizationPlan {
    pub cabinets: Vec<CabinetPlan>,
    pub movements: Vec<FileMovement>,
}

#[derive(Debug, Clone)]
pub struct CabinetPlan {
    pub name: String,
    pub description: String,
    pub shelves: Vec<ShelfPlan>,
}

#[derive(Debug, Clone)]
pub struct ShelfPlan {
    pub name: String,
    pub description: String,
    pub item_count: usize,
}

#[derive(Debug, Clone)]
pub struct FileMovement {
    pub from: PathBuf,
    pub to_cabinet: String,
    pub to_shelf: String,
    pub new_name: Option<String>,
    pub reasoning: String,
}
