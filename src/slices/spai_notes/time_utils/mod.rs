//! Calendar and timestamp utilities.
//!
//! - [`clock_parts`] — OS-local wall-clock reading (`current_timestamp_and_date`).
//! - [`short_date`] — `dd.mm.yy` rendering of any timestamp format.
mod clock_parts;
pub mod short_date;

pub use clock_parts::{current_stamp_czech, current_timestamp_and_date, CalendarMoment};
pub use short_date::format_short_date;
