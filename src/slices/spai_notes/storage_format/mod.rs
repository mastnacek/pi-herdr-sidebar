//! Parsing and formatting for SPAI notes markdown, YAML frontmatter, and 5D facets.
pub mod formatter;
pub mod parser;

#[cfg(test)]
mod tests;

pub use formatter::{format_spai_markdown, slugify, update_body_status_prefix};
pub use parser::{parse_inline_tags, parse_spai_markdown};
