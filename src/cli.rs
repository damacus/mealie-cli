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
    /// Retrieve one recipe and its ingredients by slug or exact name
    #[command(after_help = "Example:\n  mealie recipes get butter-chicken")]
    Get {
        /// Recipe slug, or an exact case-insensitive recipe name
        slug: String,
    },
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
