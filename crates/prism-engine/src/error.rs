use thiserror::Error;

/// Everything that can stop the engine from doing what it was asked. Each message is written to be shown to the user.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not read the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not read the CSV file: {0}")]
    Csv(#[from] csv::Error),
    #[error("The file has no header row, so the variables can't be named.")]
    NoHeader,
    #[error("There is no variable named \"{0}\".")]
    UnknownVariable(String),
    #[error("A variable named \"{0}\" already exists.")]
    DuplicateVariable(String),
    #[error("\"{0}\" is not a valid variable name. Names start with a letter and use letters, digits and underscores.")]
    InvalidName(String),
    #[error("The column has {got} values but the dataset has {expected} rows.")]
    RowCount { expected: usize, got: usize },
    #[error("\"{0}\" holds text, but this analysis needs numbers.")]
    NotNumeric(String),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;
