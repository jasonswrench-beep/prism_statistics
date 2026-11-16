//! The PrismStatistics engine.
//!
//! The engine holds datasets (variables with labels, value labels and missing-value codes), reads files into them, runs
//! analyses, and returns each result as a [`result::ResultBundle`]: tables, case accounting, diagnostics, and named
//! fields that report writers (the template writer here, and later the AI writer) fill text from.
//!
//! The numbers themselves come from `prismqda-stats`, the statistics crate shared with PrismQDA (decision D03), which
//! is validated against R. This crate adds what a statistics package needs around the numbers: which cases are used and
//! why others are not, labels, defaults, and honest reporting.

pub mod analysis;
pub mod apa;
pub mod chart;
pub mod data_ops;
pub mod dataset_export;
pub mod error;
pub mod export;
pub mod health;
pub mod import;
pub mod job;
pub mod limits;
pub mod model;
pub mod project;
pub mod reshape;
pub mod result;
pub mod statfile;
pub mod subset;
pub mod svg_clean;
pub mod templates;
pub mod transform;
pub mod xapi;
pub mod xlsx;

pub use error::{Error, Result};
pub use model::{Column, Dataset, Measure, ValueLabel, Variable};
pub use result::ResultBundle;
