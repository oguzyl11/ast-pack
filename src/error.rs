use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("focus files alone need {needed} tokens, which exceeds the budget of {budget}")]
    FocusExceedsBudget { needed: usize, budget: usize },
    #[error("budget of {budget} tokens cannot fit the packed header ({needed} tokens)")]
    BudgetTooSmall { needed: usize, budget: usize },
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl PackError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }
}
