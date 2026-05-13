use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "mealie", version, about = "CLI for the Mealie REST API")]
pub struct Cli {
    #[arg(long, global = true, conflicts_with_all = ["ndjson", "quiet"])]
    pub json: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "quiet"])]
    pub ndjson: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "ndjson"])]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(subcommand)]
    Recipes(RecipesCommand),
    #[command(subcommand)]
    Plan(PlanCommand),
}

#[derive(Debug, Subcommand)]
pub enum RecipesCommand {
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
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
        #[arg(long = "type")]
        meal_type: Option<String>,
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
    #[arg(long = "type")]
    pub meal_type: String,
    #[arg(long, conflicts_with = "recipe", required_unless_present = "recipe")]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "title", required_unless_present = "title")]
    pub recipe: Option<String>,
}
