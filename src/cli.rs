use clap::{Args, Parser, Subcommand};

use crate::meal_type::MealType;

#[derive(Debug, Parser)]
#[command(
    name = "mealie",
    version,
    about = "Manage recipes and meal plans in Mealie",
    after_help = "Examples:\n  mealie recipes search \"pesto chicken\"\n  mealie plan list --from 2026-05-13 --to 2026-05-16\n  mealie plan set --date 2026-05-16 --type dinner --recipe pesto-chicken"
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
    #[command(subcommand, visible_alias = "recipe")]
    Recipes(RecipesCommand),
    #[command(subcommand, visible_alias = "meal-plan")]
    Plan(PlanCommand),
}

#[derive(Debug, Subcommand)]
pub enum RecipesCommand {
    Search {
        query: String,
        #[arg(short, long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
    },
    Get {
        slug: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    List {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long = "type", value_enum)]
        meal_type: Option<MealType>,
    },
    Set(PlanSetArgs),
    Delete {
        #[arg(long)]
        id: i64,
    },
}

#[derive(Debug, Args)]
pub struct PlanSetArgs {
    #[arg(long)]
    pub date: String,
    #[arg(long = "type", value_enum)]
    pub meal_type: MealType,
    #[arg(long, conflicts_with = "recipe", required_unless_present = "recipe")]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "title", required_unless_present = "title")]
    pub recipe: Option<String>,
}
