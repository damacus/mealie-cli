use crate::{AppError, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MealType(String);

impl MealType {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "breakfast" | "lunch" | "dinner" | "side" | "snack" | "drink" | "dessert" => {
                Ok(Self(value.to_string()))
            }
            _ => Err(AppError::new(
                ErrorCode::InvalidArgs,
                format!("invalid meal type: {value}"),
            )),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
