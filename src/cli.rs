use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

use crate::meal_type::MealType;

#[derive(Debug, Parser)]
#[command(
    name = "mealie",
    version,
    about = "Manage recipes and meal plans in Mealie",
    arg_required_else_help = true,
    after_help = "Examples:\n  mealie status\n  mealie recipes search \"pesto chicken\"\n  mealie plan list --from 2026-05-13 --to 2026-05-16\n  mealie plan week --offset -1\n  mealie plan set --date 2026-05-16 --type dinner --recipe pesto-chicken"
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
    /// List recipe tags
    #[command(subcommand)]
    Tags(OrganizerCommand),
    /// List recipe categories
    #[command(subcommand)]
    Categories(OrganizerCommand),
    /// List, create, replace, or delete meal plan entries
    #[command(subcommand, visible_alias = "meal-plan")]
    Plan(PlanCommand),
}

#[derive(Debug, Subcommand)]
pub enum OrganizerCommand {
    /// List all organisers
    List {
        #[arg(long)]
        empty: bool,
    },
    /// Get an organiser by ID or slug
    #[command(group(clap::ArgGroup::new("organizer_identifier").required(true).args(["id", "slug"])))]
    Get {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        slug: Option<String>,
    },
    /// Rename an organiser by ID
    Rename {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Merge an organiser into another, removing the source
    Merge {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete an organiser
    Delete {
        #[arg(long)]
        id: String,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Create an organiser
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RecipesCommand {
    /// Update several recipes from a JSON array; replacement can reset omitted fields
    BatchUpdate {
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Replace a recipe from a JSON object (omitted fields may be reset)
    Replace {
        slug: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Queue several recipe URL imports
    BulkImport {
        #[arg(long, required = true)]
        url: Vec<String>,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        category: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Inspect a bulk import report by ID
    ImportReport {
        #[arg(long)]
        id: String,
    },
    /// Duplicate a recipe, optionally choosing a name
    Duplicate {
        slug: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Set the last-made timestamp
    LastMade {
        slug: String,
        #[arg(long)]
        timestamp: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Suggest recipes using available food and tools
    Suggest {
        #[arg(long)]
        food: Vec<String>,
        #[arg(long)]
        tool: Vec<String>,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
        #[arg(long)]
        max_missing_foods: Option<u32>,
        #[arg(long)]
        max_missing_tools: Option<u32>,
        #[arg(long)]
        exclude_foods_on_hand: bool,
        #[arg(long)]
        exclude_tools_on_hand: bool,
        #[arg(long)]
        exclude_substitutions: bool,
    },
    /// Manage bulk recipe exports
    #[command(subcommand)]
    BulkExport(BulkExportCommand),
    /// Replace settings on several recipes, preserving each lock state
    BulkSettings {
        #[arg(long, required = true)]
        recipe: Vec<String>,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete several recipes
    BulkDelete {
        #[arg(long, required = true)]
        recipe: Vec<String>,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add tags to several recipes
    BulkTag {
        #[arg(long, required = true)]
        recipe: Vec<String>,
        #[arg(long, required = true)]
        tag: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add categories to several recipes
    BulkCategorize {
        #[arg(long, required = true)]
        recipe: Vec<String>,
        #[arg(long, required = true)]
        category: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Import a recipe from a URL
    ImportUrl {
        #[arg(long)]
        url: String,
        #[arg(long)]
        include_tags: bool,
        #[arg(long)]
        include_categories: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Import a recipe from HTML or JSON
    ImportFile {
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        include_tags: bool,
        #[arg(long)]
        include_categories: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Import a recipe from a Mealie ZIP archive
    ImportZip {
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Import a recipe using the server's AI integration
    ImportAi {
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        image: Vec<std::path::PathBuf>,
        #[arg(long)]
        translate_language: Option<String>,
        #[arg(long)]
        create_new_organizers: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// List available export templates
    ExportFormats,
    /// Export a recipe to a new file
    Export {
        slug: String,
        #[arg(long)]
        template: String,
        #[arg(long)]
        output: std::path::PathBuf,
    },
    /// Update recipe fields from a JSON object
    Patch {
        slug: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Create an empty recipe
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a recipe
    Delete {
        slug: String,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage a recipe image
    #[command(subcommand)]
    Image(RecipeImageCommand),
    /// Add or remove a tag or category by ID
    #[command(group(clap::ArgGroup::new("organizer_change").required(true).args(["add_tag", "remove_tag", "add_category", "remove_category"])))]
    Organize {
        slug: String,
        #[arg(long)]
        add_tag: Option<String>,
        #[arg(long)]
        remove_tag: Option<String>,
        #[arg(long)]
        add_category: Option<String>,
        #[arg(long)]
        remove_category: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Update a recipe description
    Update {
        slug: String,
        #[arg(long)]
        description: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// List all recipes, optionally filtering missing organisers
    List {
        #[arg(long)]
        untagged: bool,
        #[arg(long)]
        uncategorised: bool,
    },
    /// Search recipes by name or ingredient
    #[command(after_help = "Examples:\n  mealie recipes search \"pesto chicken\" --limit 5")]
    Search {
        /// Search text, for example "pesto chicken"
        query: String,
        #[arg(short, long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100), help = "Maximum number of recipes to return (1-100)")]
        limit: u32,
    },
    /// Retrieve one recipe and its ingredients by slug or exact name
    #[command(after_help = "Example:\n  mealie recipes get butter-chicken")]
    Get {
        /// Recipe slug, or an exact case-insensitive recipe name
        slug: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum BulkExportCommand {
    /// Request an export for selected recipes
    Start {
        #[arg(long, required = true)]
        recipe: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// List available exports
    List,
    /// Download a completed export to a new file
    Download {
        #[arg(long)]
        id: String,
        #[arg(long)]
        output: std::path::PathBuf,
    },
    /// Purge all group exports
    Purge {
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RecipeImageCommand {
    /// Upload an image from a local file
    Upload {
        slug: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Download a recipe image without overwriting an existing file
    Get {
        slug: String,
        #[arg(long)]
        output: std::path::PathBuf,
        #[arg(long, value_enum, default_value_t = RecipeImageSize::Original)]
        size: RecipeImageSize,
    },
    /// Set an image from a direct URL
    SetUrl {
        slug: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete the recipe image
    Delete {
        slug: String,
        #[arg(long, conflicts_with = "dry_run")]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum RecipeImageSize {
    Original,
    Small,
    Tiny,
}

impl RecipeImageSize {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Original => "original.webp",
            Self::Small => "min-original.webp",
            Self::Tiny => "tiny-original.webp",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// List meal plan entries for this week or a date range
    #[command(
        after_help = "Examples:\n  mealie plan list\n  mealie plan list --from today --to +3d\n  mealie plan list --from 2026-05-13 --to 2026-05-16 --type dinner\n\nDates accept YYYY-MM-DD, today, tomorrow, yesterday, +Nd/-Nd, or +Nw/-Nw.\nWith no dates, the range is today through Sunday in your local timezone."
    )]
    List {
        /// First date to include (defaults to today; accepts YYYY-MM-DD or relative dates)
        #[arg(long, allow_hyphen_values = true)]
        from: Option<String>,
        /// Last date to include (defaults to this week's Sunday; accepts YYYY-MM-DD or relative dates)
        #[arg(long, allow_hyphen_values = true)]
        to: Option<String>,
        /// Only return entries for this meal type
        #[arg(long = "type", value_enum)]
        meal_type: Option<MealType>,
    },
    /// Show a Monday-to-Sunday meal-plan view
    #[command(
        after_help = "Examples:\n  mealie plan week\n  mealie plan week --date 2026-05-13\n  mealie plan week --offset -1\n\n--date accepts YYYY-MM-DD, today, tomorrow, yesterday, +Nd/-Nd, or +Nw/-Nw.\n--offset is a signed number of whole weeks from the current local week.\n--date and --offset cannot be used together."
    )]
    Week {
        /// Anchor the view to the ISO week containing this date
        #[arg(long, allow_hyphen_values = true, conflicts_with = "offset")]
        date: Option<String>,
        /// Signed number of weeks from the current local week
        #[arg(
            long,
            allow_hyphen_values = true,
            value_name = "WEEKS",
            conflicts_with = "date"
        )]
        offset: Option<i64>,
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
    /// Date to plan (YYYY-MM-DD, today, tomorrow, yesterday, +Nd/-Nd, or +Nw/-Nw)
    #[arg(long, allow_hyphen_values = true)]
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
    /// Recipe slug, or an exact case-insensitive recipe name to add to the plan
    #[arg(long)]
    pub recipe: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn parses_space_separated_negative_relative_values() {
        let cli = Cli::try_parse_from(["mealie", "plan", "list", "--from", "-1d", "--to", "-1w"])
            .expect("negative relative list dates should parse");

        let Command::Plan(PlanCommand::List { from, to, .. }) = cli.command else {
            panic!("expected plan list command");
        };
        assert_eq!(from.as_deref(), Some("-1d"));
        assert_eq!(to.as_deref(), Some("-1w"));

        let cli = Cli::try_parse_from([
            "mealie", "plan", "set", "--date", "-1d", "--type", "dinner", "--title", "Soup",
        ])
        .expect("negative relative set date should parse");

        let Command::Plan(PlanCommand::Set(args)) = cli.command else {
            panic!("expected plan set command");
        };
        assert_eq!(args.date, "-1d");
    }

    #[test]
    fn rejects_unknown_flags() {
        let error = Cli::try_parse_from(["mealie", "plan", "list", "--unknown"])
            .expect_err("unknown flags must remain rejected");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn parses_week_options_and_rejects_mutual_exclusion() {
        let cli = Cli::try_parse_from(["mealie", "plan", "week", "--offset", "-1"])
            .expect("negative week offsets should parse");
        let Command::Plan(PlanCommand::Week { date, offset }) = cli.command else {
            panic!("expected plan week command");
        };
        assert_eq!(date, None);
        assert_eq!(offset, Some(-1));

        let cli = Cli::try_parse_from(["mealie", "plan", "week", "--date", "-1w"])
            .expect("negative date anchors should parse");
        let Command::Plan(PlanCommand::Week { date, offset }) = cli.command else {
            panic!("expected plan week command");
        };
        assert_eq!(date.as_deref(), Some("-1w"));
        assert_eq!(offset, None);

        let error = Cli::try_parse_from([
            "mealie",
            "plan",
            "week",
            "--date",
            "2026-05-13",
            "--offset",
            "1",
        ])
        .expect_err("week date and offset should conflict");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }
}
