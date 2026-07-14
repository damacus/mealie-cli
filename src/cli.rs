use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

use crate::meal_type::MealType;

#[derive(Debug, Parser)]
#[command(
    name = "mealie",
    version,
    about = "Manage recipes and meal plans in Mealie",
    arg_required_else_help = true,
    after_help = "Examples:\n  mealie status\n  mealie recipes search \"pesto chicken\"\n  mealie plan list --from 2026-05-13 --to 2026-05-16\n  mealie plan set --date 2026-05-16 --type dinner --recipe pesto-chicken"
)]
pub struct Cli {
    #[arg(long, global = true, conflicts_with_all = ["ndjson", "quiet"], help = "Output one pretty JSON array")]
    pub json: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "quiet"], help = "Output one JSON object per line")]
    pub ndjson: bool,
    #[arg(short, long, global = true, conflicts_with_all = ["json", "ndjson"], help = "Quiet mode; print only IDs for successful changes")]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate shell-completion script
    Completion {
        /// Shell to generate completions for
        shell: Shell,
    },
    /// Check configuration, connectivity, and authentication
    Status,
    /// Search for or retrieve recipes
    #[command(subcommand, visible_alias = "recipe")]
    Recipes(RecipesCommand),
    /// List, create, replace, or delete meal plan entries
    #[command(subcommand, visible_alias = "meal-plan")]
    Plan(PlanCommand),
}

#[derive(Debug, Subcommand)]
pub enum RecipesCommand {
    /// Search recipes by name or ingredient
    #[command(after_help = "Examples:\n  mealie recipes search \"pesto chicken\" --limit 5")]
    Search {
        /// Search text, for example "pesto chicken"
        query: String,
        #[arg(short, long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100), help = "Maximum number of recipes to return (1-100)")]
        limit: u32,
    },
    /// Retrieve one recipe and its ingredients by exact slug
    #[command(after_help = "Example:\n  mealie recipes get butter-chicken")]
    Get {
        /// Exact recipe slug, for example "butter-chicken"
        slug: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// List meal plan entries in a date range
    #[command(
        after_help = "Examples:\n  mealie plan list --from 2026-05-13 --to 2026-05-16\n  mealie plan list --from 2026-05-13 --to 2026-05-16 --type dinner"
    )]
    List {
        /// First date to include, in YYYY-MM-DD format
        #[arg(long)]
        from: String,
        /// Last date to include, in YYYY-MM-DD format
        #[arg(long)]
        to: String,
        /// Only return entries for this meal type
        #[arg(long = "type", value_enum)]
        meal_type: Option<MealType>,
    },
    /// Create or replace a meal plan entry
    #[command(
        after_help = "Examples:\n  mealie plan set --date 2026-05-13 --type dinner --title \"Bolognaise\"\n\nCreate a dinner plan entry from a recipe:\n  mealie plan set --date 2026-05-16 --type dinner --recipe pesto-chicken"
    )]
    Set(PlanSetArgs),
    /// Delete a meal plan entry by ID
    #[command(after_help = "Example:\n  mealie plan delete --id 123")]
    Delete {
        /// ID of the meal plan entry to delete
        #[arg(long)]
        id: i64,
    },
}

#[derive(Debug, Args)]
pub struct PlanSetArgs {
    /// Date to plan, in YYYY-MM-DD format
    #[arg(long)]
    pub date: String,
    /// Meal type to plan
    #[arg(long = "type", value_enum)]
    pub meal_type: MealType,
    #[command(flatten)]
    pub target: PlanSetTargetArgs,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct PlanSetTargetArgs {
    /// Plain-text meal title
    #[arg(long)]
    pub title: Option<String>,
    /// Exact recipe slug to add to the plan
    #[arg(long)]
    pub recipe: Option<String>,
}
