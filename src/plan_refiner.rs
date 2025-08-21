use anyhow::{Context, Result};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Input};
use rig::{
    completion::{request::ToolDefinition, Prompt},
    prelude::*,
    tool::Tool,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::{path::PathBuf, sync::Arc};

use crate::{
    database::Database,
    models::OrganizationPlan,
    providers::{LLMProvider, Provider},
};

#[derive(Debug, thiserror::Error)]
pub enum PlanToolError {
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Item not found: {0}")]
    ItemNotFound(String),
    #[error("Cabinet not found: {0}")]
    CabinetNotFound(String),
    #[error("Shelf not found: {0}")]
    ShelfNotFound(String),
}

#[derive(Deserialize, Serialize)]
pub struct MoveItemArgs {
    pub item_id: i64,
    pub target_cabinet_name: String,
    pub target_shelf_name: String,
}

#[derive(Deserialize, Serialize)]
pub struct CreateCabinetArgs {
    pub name: String,
    pub description: String,
}

#[derive(Deserialize, Serialize)]
pub struct CreateShelfArgs {
    pub cabinet_name: String,
    pub name: String,
    pub description: String,
}

#[derive(Deserialize, Serialize)]
pub struct RenameCabinetArgs {
    pub current_name: String,
    pub new_name: String,
    pub new_description: String,
}

#[derive(Deserialize, Serialize)]
pub struct RenameShelfArgs {
    pub cabinet_name: String,
    pub current_shelf_name: String,
    pub new_name: String,
    pub new_description: String,
}

#[derive(Deserialize, Serialize)]
pub struct DeleteCabinetArgs {
    pub name: String,
}

#[derive(Deserialize, Serialize)]
pub struct DeleteShelfArgs {
    pub cabinet_name: String,
    pub shelf_name: String,
}

#[derive(Deserialize, Serialize)]
pub struct ListItemsArgs {}

#[derive(Deserialize, Serialize)]
pub struct ListCabinetsArgs {}

pub struct PlanRefiner {
    provider: LLMProvider,
    database: Arc<Database>,
    base_path: PathBuf,
}

// Tool definitions
pub struct MoveItemTool {
    database: Arc<Database>,
}

impl Tool for MoveItemTool {
    const NAME: &'static str = "move_item";
    type Error = PlanToolError;
    type Args = MoveItemArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "move_item".to_string(),
            description: "Move an item to a different cabinet and shelf".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {
                        "type": "integer",
                        "description": "The ID of the item to move"
                    },
                    "target_cabinet_name": {
                        "type": "string",
                        "description": "The name of the target cabinet"
                    },
                    "target_shelf_name": {
                        "type": "string",
                        "description": "The name of the target shelf"
                    }
                },
                "required": ["item_id", "target_cabinet_name", "target_shelf_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Find the target cabinet and shelf
        let cabinet = self
            .database
            .get_cabinet_by_name(&args.target_cabinet_name)?
            .ok_or_else(|| PlanToolError::CabinetNotFound(args.target_cabinet_name.clone()))?;

        let shelf = self
            .database
            .get_shelf_by_name(cabinet.id, &args.target_shelf_name)?
            .ok_or_else(|| PlanToolError::ShelfNotFound(args.target_shelf_name.clone()))?;

        // Move the item
        self.database.update_item_shelf(args.item_id, shelf.id)?;

        Ok(format!(
            "Successfully moved item {} to cabinet '{}', shelf '{}'",
            args.item_id, args.target_cabinet_name, args.target_shelf_name
        ))
    }
}

pub struct CreateCabinetTool {
    database: Arc<Database>,
}

impl Tool for CreateCabinetTool {
    const NAME: &'static str = "create_cabinet";
    type Error = PlanToolError;
    type Args = CreateCabinetArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_cabinet".to_string(),
            description: "Create a new cabinet for organizing files".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the new cabinet"
                    },
                    "description": {
                        "type": "string",
                        "description": "A description of what this cabinet will contain"
                    }
                },
                "required": ["name", "description"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let cabinet_id = self
            .database
            .create_cabinet(&args.name, &args.description)?;
        Ok(format!(
            "Successfully created cabinet '{}' (ID: {}) - {}",
            args.name, cabinet_id, args.description
        ))
    }
}

pub struct CreateShelfTool {
    database: Arc<Database>,
}

impl Tool for CreateShelfTool {
    const NAME: &'static str = "create_shelf";
    type Error = PlanToolError;
    type Args = CreateShelfArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_shelf".to_string(),
            description: "Create a new shelf within an existing cabinet".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "cabinet_name": {
                        "type": "string",
                        "description": "The name of the cabinet to add the shelf to"
                    },
                    "name": {
                        "type": "string",
                        "description": "The name of the new shelf"
                    },
                    "description": {
                        "type": "string",
                        "description": "A description of what this shelf will contain"
                    }
                },
                "required": ["cabinet_name", "name", "description"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let cabinet = self
            .database
            .get_cabinet_by_name(&args.cabinet_name)?
            .ok_or_else(|| PlanToolError::CabinetNotFound(args.cabinet_name.clone()))?;

        let shelf_id = self
            .database
            .create_shelf(cabinet.id, &args.name, &args.description)?;
        Ok(format!(
            "Successfully created shelf '{}' (ID: {}) in cabinet '{}' - {}",
            args.name, shelf_id, args.cabinet_name, args.description
        ))
    }
}

pub struct ListItemsTool {
    database: Arc<Database>,
}

impl Tool for ListItemsTool {
    const NAME: &'static str = "list_items";
    type Error = PlanToolError;
    type Args = ListItemsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_items".to_string(),
            description:
                "List all items in the database with their current cabinet and shelf assignments"
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let items = self.database.list_all_items()?;
        let cabinets = self.database.list_cabinets()?;
        let shelves = self.database.list_shelves(None)?;

        let mut result = String::new();
        result.push_str("Current items in the database:\n\n");

        for item in items {
            let shelf = shelves
                .iter()
                .find(|s| s.id == item.shelf_id)
                .ok_or_else(|| PlanToolError::ShelfNotFound(format!("ID {}", item.shelf_id)))?;
            let cabinet = cabinets
                .iter()
                .find(|c| c.id == shelf.cabinet_id)
                .ok_or_else(|| {
                    PlanToolError::CabinetNotFound(format!("ID {}", shelf.cabinet_id))
                })?;

            result.push_str(&format!(
                "Item {} (ID: {}): {} - Located in Cabinet '{}' / Shelf '{}'\n  Description: {}\n  File type: {}\n\n",
                item.original_name,
                item.id.unwrap_or(0),
                item.path,
                cabinet.name,
                shelf.name,
                item.description,
                item.file_type
            ));
        }

        Ok(result)
    }
}

pub struct ListCabinetsTool {
    database: Arc<Database>,
}

impl Tool for ListCabinetsTool {
    const NAME: &'static str = "list_cabinets";
    type Error = PlanToolError;
    type Args = ListCabinetsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_cabinets".to_string(),
            description: "List all cabinets and their shelves in the current organization"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let cabinets = self.database.list_cabinets()?;
        let shelves = self.database.list_shelves(None)?;

        let mut result = String::new();
        result.push_str("Current cabinet and shelf structure:\n\n");

        for cabinet in cabinets {
            result.push_str(&format!(
                "Cabinet '{}' (ID: {}): {}\n",
                cabinet.name, cabinet.id, cabinet.description
            ));

            let cabinet_shelves: Vec<_> = shelves
                .iter()
                .filter(|s| s.cabinet_id == cabinet.id)
                .collect();
            for shelf in cabinet_shelves {
                result.push_str(&format!(
                    "  - Shelf '{}' (ID: {}): {}\n",
                    shelf.name, shelf.id, shelf.description
                ));
            }
            result.push('\n');
        }

        Ok(result)
    }
}

impl PlanRefiner {
    pub fn new(provider: LLMProvider, database: Arc<Database>, base_path: PathBuf) -> Self {
        Self {
            provider,
            database,
            base_path,
        }
    }

    pub async fn refine_plan_with_feedback(
        &self,
        current_plan: &OrganizationPlan,
    ) -> Result<Option<OrganizationPlan>> {
        loop {
            // Get user feedback
            let user_feedback = self.get_user_feedback()?;

            if user_feedback.trim().is_empty() || user_feedback.trim().to_lowercase() == "exit" {
                println!("{}", "Exiting refinement mode.".yellow());
                return Ok(None);
            }

            println!(
                "\n{}",
                "Analyzing feedback and refining plan...".cyan().bold()
            );

            // Create agent with database tools
            match self.refine_with_agent(&user_feedback, current_plan).await {
                Ok(_) => {
                    // Generate new plan from updated database
                    let new_plan = self.create_updated_organization_plan()?;

                    println!("\n{}", "Revised Organization Plan:".cyan().bold());
                    self.print_plan(&new_plan)?;

                    // Ask if user accepts the revised plan
                    if self.get_plan_approval()? {
                        return Ok(Some(new_plan));
                    } else {
                        println!("\n{}", "Let's continue refining the plan.".yellow());
                        // Loop continues to get more feedback
                    }
                }
                Err(e) => {
                    eprintln!("{}: Failed to refine plan: {}", "Error".red().bold(), e);
                    println!("Let's try again with different feedback.");
                }
            }
        }
    }

    async fn refine_with_agent(
        &self,
        user_feedback: &str,
        _current_plan: &OrganizationPlan,
    ) -> Result<()> {
        // Create tools with database access
        let move_item_tool = MoveItemTool {
            database: Arc::clone(&self.database),
        };
        let create_cabinet_tool = CreateCabinetTool {
            database: Arc::clone(&self.database),
        };
        let create_shelf_tool = CreateShelfTool {
            database: Arc::clone(&self.database),
        };
        let list_items_tool = ListItemsTool {
            database: Arc::clone(&self.database),
        };
        let list_cabinets_tool = ListCabinetsTool {
            database: Arc::clone(&self.database),
        };

        let initial_prompt = format!(
            r#"You are a file organization assistant helping to refine a file organization plan based on user feedback.

The user has provided the following feedback about their current organization plan:
"{}"

You have access to tools that let you:
1. List current cabinets and shelves (list_cabinets)
2. List all items and their current locations (list_items)
3. Move items between cabinets/shelves (move_item)
4. Create new cabinets (create_cabinet)
5. Create new shelves within cabinets (create_shelf)

Your task:
1. First, understand the current organization by listing cabinets and items
2. Based on the user's feedback, determine what changes need to be made
3. Use the available tools to implement those changes
4. Provide a clear explanation of what you did and why

Please start by examining the current organization structure."#,
            user_feedback
        );

        match self.provider.get_provider() {
            Provider::OpenAI => {
                let client = self.provider.get_openai_client()?;
                let agent = client
                    .agent(self.provider.get_model_name())
                    .preamble(&initial_prompt)
                    .max_tokens(2000)
                    .tool(list_cabinets_tool)
                    .tool(list_items_tool)
                    .tool(move_item_tool)
                    .tool(create_cabinet_tool)
                    .tool(create_shelf_tool)
                    .build();

                let response = agent.prompt("Please examine the current organization and implement the requested changes.").multi_turn(20).await?;
                println!("\n{}", "Agent Response:".green().bold());
                println!("{}", response);
            }
            Provider::Anthropic => {
                let client = self.provider.get_anthropic_client()?;
                let agent = client
                    .agent(self.provider.get_model_name())
                    .preamble(&initial_prompt)
                    .max_tokens(2000)
                    .tool(list_cabinets_tool)
                    .tool(list_items_tool)
                    .tool(move_item_tool)
                    .tool(create_cabinet_tool)
                    .tool(create_shelf_tool)
                    .build();

                let response = agent.prompt("Please examine the current organization and implement the requested changes.").multi_turn(20).await?;
                println!("\n{}", "Agent Response:".green().bold());
                println!("{}", response);
            }
            Provider::Ollama => {
                let client = self.provider.get_ollama_client()?;
                let agent = client
                    .agent(self.provider.get_model_name())
                    .preamble(&initial_prompt)
                    .max_tokens(2000)
                    .tool(list_cabinets_tool)
                    .tool(list_items_tool)
                    .tool(move_item_tool)
                    .tool(create_cabinet_tool)
                    .tool(create_shelf_tool)
                    .build();

                let response = agent.prompt("Please examine the current organization and implement the requested changes.").multi_turn(20).await?;
                println!("\n{}", "Agent Response:".green().bold());
                println!("{}", response);
            }
        }

        Ok(())
    }

    fn get_user_feedback(&self) -> Result<String> {
        println!("\n{}", "Plan Refinement".cyan().bold());
        println!("Please describe what you'd like to change about the organization plan.");
        println!("Examples:");
        println!("  - \"Move all image files to a Photography cabinet\"");
        println!("  - \"Create separate shelves for different programming languages\"");
        println!("  - \"Rename the Documents cabinet to Personal Files\"");
        println!("  - \"Group all video files together regardless of format\"");
        println!("Type 'exit' to cancel.\n");

        let feedback: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to change?")
            .interact_text()?;

        Ok(feedback)
    }

    fn get_plan_approval(&self) -> Result<bool> {
        use dialoguer::Confirm;

        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Do you approve this revised plan?")
            .default(false)
            .interact()
            .context("Failed to get user confirmation")
    }

    fn create_updated_organization_plan(&self) -> Result<OrganizationPlan> {
        // This is the same logic as in organizer.rs create_organization_plan
        let cabinets = self.database.list_cabinets()?;
        let shelves = self.database.list_shelves(None)?;
        let items = self.database.list_all_items()?;

        let mut cabinet_plans = Vec::new();

        for cabinet in &cabinets {
            let cabinet_shelves = shelves
                .iter()
                .filter(|s| s.cabinet_id == cabinet.id)
                .collect::<Vec<_>>();

            let mut shelf_plans = Vec::new();

            for shelf in cabinet_shelves {
                let item_count = items.iter().filter(|i| i.shelf_id == shelf.id).count();

                shelf_plans.push(crate::models::ShelfPlan {
                    name: shelf.name.clone(),
                    description: shelf.description.clone(),
                    item_count,
                });
            }

            cabinet_plans.push(crate::models::CabinetPlan {
                name: cabinet.name.clone(),
                description: cabinet.description.clone(),
                shelves: shelf_plans,
            });
        }

        let mut movements = Vec::new();

        for item in items {
            let shelf = shelves
                .iter()
                .find(|s| s.id == item.shelf_id)
                .context("Shelf not found for item")?;

            let cabinet = cabinets
                .iter()
                .find(|c| c.id == shelf.cabinet_id)
                .context("Cabinet not found for shelf")?;

            let from = PathBuf::from(&item.path);

            movements.push(crate::models::FileMovement {
                from: from.clone(),
                to_cabinet: cabinet.name.clone(),
                to_shelf: shelf.name.clone(),
                new_name: item.suggested_name.clone(),
                reasoning: item.description.clone(),
            });
        }

        Ok(OrganizationPlan {
            cabinets: cabinet_plans,
            movements,
        })
    }

    fn print_plan(&self, plan: &OrganizationPlan) -> Result<()> {
        println!("\n{}", "Cabinet Structure:".cyan());

        for cabinet in &plan.cabinets {
            println!(
                "  🗄  {} - {}",
                cabinet.name.blue().bold(),
                cabinet.description
            );

            for shelf in &cabinet.shelves {
                println!(
                    "      📁 {} ({} items) - {}",
                    shelf.name.green(),
                    shelf.item_count,
                    shelf.description.dimmed()
                );
            }
        }

        println!("\n{}", "File Movements:".cyan());

        let display_limit = 10;
        let total = plan.movements.len();

        for (idx, movement) in plan.movements.iter().take(display_limit).enumerate() {
            let from_name = movement
                .from
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let default_name = from_name.to_string();
            let to_name = movement.new_name.as_ref().unwrap_or(&default_name);

            println!(
                "  {} → {}/{}/{}",
                from_name.yellow(),
                movement.to_cabinet.blue(),
                movement.to_shelf.green(),
                if movement.new_name.is_some() {
                    to_name.cyan().to_string()
                } else {
                    to_name.to_string()
                }
            );

            if idx < 3 || total <= display_limit {
                println!("    {}", movement.reasoning.dimmed());
            }
        }

        if total > display_limit {
            println!("  ... and {} more files", total - display_limit);
        }

        Ok(())
    }
}
