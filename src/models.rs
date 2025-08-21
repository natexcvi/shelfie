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
    #[schemars(
        description = "Analysis results for each item in the batch, in the same order as the input items"
    )]
    pub items: Vec<ItemAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ItemAnalysis {
    #[schemars(description = "Must match the id from the corresponding input item")]
    pub id: String,
    #[schemars(
        description = "Brief one-sentence description of what this item contains or represents"
    )]
    pub description: String,
    #[schemars(
        description = "Better name for the item if current name needs improvement, or empty string if current name is fine"
    )]
    pub suggested_name: String,
    #[schemars(
        description = "For directories only: true if directory contains homogeneous/generated content that should be treated as a single unit"
    )]
    pub is_opaque_directory: bool,
    pub cabinet: CabinetAssignment,
    pub shelf: ShelfAssignment,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CabinetAssignment {
    #[schemars(
        description = "Must be exactly 'existing' to use an existing cabinet or 'new' to create a new one"
    )]
    pub assignment_type: String,
    #[schemars(
        description = "When assignment_type is 'existing': must be the ID of an existing cabinet from the input. When assignment_type is 'new': must be 0"
    )]
    pub existing_id: i64,
    #[schemars(
        description = "When assignment_type is 'new': short descriptive name for the new cabinet. When assignment_type is 'existing': must be empty string"
    )]
    pub new_name: String,
    #[schemars(
        description = "When assignment_type is 'new': detailed description of what the new cabinet will contain. When assignment_type is 'existing': must be empty string"
    )]
    pub new_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ShelfAssignment {
    #[schemars(
        description = "Must be exactly 'existing' to use an existing shelf or 'new' to create a new one"
    )]
    pub assignment_type: String,
    #[schemars(
        description = "When assignment_type is 'existing': must be the ID of an existing shelf from the input. When assignment_type is 'new': must be 0"
    )]
    pub existing_id: i64,
    #[schemars(
        description = "When assignment_type is 'new': short descriptive name for the new shelf within its cabinet. When assignment_type is 'existing': must be empty string"
    )]
    pub new_name: String,
    #[schemars(
        description = "When assignment_type is 'new': detailed description of what the new shelf will contain. When assignment_type is 'existing': must be empty string"
    )]
    pub new_description: String,
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
