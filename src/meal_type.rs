use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
    Side,
    Snack,
    Drink,
    Dessert,
}

impl MealType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Breakfast => "breakfast",
            Self::Lunch => "lunch",
            Self::Dinner => "dinner",
            Self::Side => "side",
            Self::Snack => "snack",
            Self::Drink => "drink",
            Self::Dessert => "dessert",
        }
    }
}
