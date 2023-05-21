#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::TemplateApp;
mod edit_distance;
pub use edit_distance::levenshtein_distance;
