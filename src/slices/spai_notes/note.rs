//! SPAI data types and prefix definitions.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpaiType {
    Todo,
    Idea,
    Note,
}

impl SpaiType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpaiType::Todo => "Todo",
            SpaiType::Idea => "Idea",
            SpaiType::Note => "Note",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "idea" => SpaiType::Idea,
            "note" => SpaiType::Note,
            _ => SpaiType::Todo,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpaiStatus {
    Todo,
    Working,
    Waiting,
    Done,
    Cancelled,
    Idea,
    Note,
    Inbox,
    Win,
    Fuckup,
    Skutek,
    Mood,
    Sleep,
    Shopping,
    Health,
    Tally,
}

impl SpaiStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpaiStatus::Todo => "todo",
            SpaiStatus::Working => "working",
            SpaiStatus::Waiting => "waiting",
            SpaiStatus::Done => "done",
            SpaiStatus::Cancelled => "cancelled",
            SpaiStatus::Idea => "idea",
            SpaiStatus::Note => "note",
            SpaiStatus::Inbox => "inbox",
            SpaiStatus::Win => "win",
            SpaiStatus::Fuckup => "fuckup",
            SpaiStatus::Skutek => "skutek",
            SpaiStatus::Mood => "mood",
            SpaiStatus::Sleep => "sleep",
            SpaiStatus::Shopping => "shopping",
            SpaiStatus::Health => "health",
            SpaiStatus::Tally => "tally",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "working" => SpaiStatus::Working,
            "waiting" => SpaiStatus::Waiting,
            "done" => SpaiStatus::Done,
            "cancelled" | "canceled" => SpaiStatus::Cancelled,
            "idea" => SpaiStatus::Idea,
            "note" => SpaiStatus::Note,
            "inbox" => SpaiStatus::Inbox,
            "win" => SpaiStatus::Win,
            "fuckup" => SpaiStatus::Fuckup,
            "skutek" => SpaiStatus::Skutek,
            "mood" => SpaiStatus::Mood,
            "sleep" => SpaiStatus::Sleep,
            "shopping" => SpaiStatus::Shopping,
            "health" => SpaiStatus::Health,
            "tally" => SpaiStatus::Tally,
            _ => SpaiStatus::Todo,
        }
    }

    pub fn next_cycle(&self) -> Self {
        match self {
            SpaiStatus::Todo => SpaiStatus::Working,
            SpaiStatus::Working => SpaiStatus::Waiting,
            SpaiStatus::Waiting => SpaiStatus::Done,
            SpaiStatus::Done => SpaiStatus::Cancelled,
            SpaiStatus::Cancelled => SpaiStatus::Todo,
            SpaiStatus::Idea => SpaiStatus::Todo,
            SpaiStatus::Note
            | SpaiStatus::Inbox
            | SpaiStatus::Win
            | SpaiStatus::Fuckup
            | SpaiStatus::Skutek
            | SpaiStatus::Mood
            | SpaiStatus::Sleep
            | SpaiStatus::Shopping
            | SpaiStatus::Health
            | SpaiStatus::Tally => SpaiStatus::Todo,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            SpaiStatus::Todo => ".",
            SpaiStatus::Working => "/",
            SpaiStatus::Waiting => "/.",
            SpaiStatus::Done => "x",
            SpaiStatus::Cancelled => "z",
            SpaiStatus::Idea => "?",
            SpaiStatus::Note => "-",
            SpaiStatus::Inbox => "#",
            SpaiStatus::Win => "*",
            SpaiStatus::Fuckup => "%",
            SpaiStatus::Skutek => "+",
            SpaiStatus::Mood => "=",
            SpaiStatus::Sleep => "~",
            SpaiStatus::Shopping => "$",
            SpaiStatus::Health => "♥",
            SpaiStatus::Tally => "#",
        }
    }

    pub fn glyph(&self) -> &'static str {
        match self {
            SpaiStatus::Todo => "[.]",
            SpaiStatus::Working => "[/]",
            SpaiStatus::Waiting => "[/.]",
            SpaiStatus::Done => "[x]",
            SpaiStatus::Cancelled => "[z]",
            SpaiStatus::Idea => "[?]",
            SpaiStatus::Note => "[-]",
            SpaiStatus::Inbox => "[#]",
            SpaiStatus::Win => "[*]",
            SpaiStatus::Fuckup => "[%]",
            SpaiStatus::Skutek => "[+]",
            SpaiStatus::Mood => "[=]",
            SpaiStatus::Sleep => "[~]",
            SpaiStatus::Shopping => "[$]",
            SpaiStatus::Health => "[♥]",
            SpaiStatus::Tally => "[#]",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpaiFacets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urgency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub who: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SpaiNoteItem {
    pub id: String,
    pub title: String,
    pub kind: SpaiType,
    pub status: SpaiStatus,
    pub symbol: String,
    pub timestamp: String,
    pub tags: Vec<String>,
    pub facets: SpaiFacets,
    pub body: String,
    pub file_path: PathBuf,
    pub file_name: String,
}

impl SpaiNoteItem {
    /// Formatted creation date in `dd.mm.yy` format.
    pub fn short_date(&self) -> String {
        super::time_utils::format_short_date(&self.timestamp)
            .or_else(|| super::time_utils::format_short_date(&self.file_name))
            .unwrap_or_else(|| "--.--.--".to_string())
    }
}

/// Matches standard SPAI prefix on raw lines (. / /. x z ? -)
pub fn parse_spai_prefix(line: &str) -> Option<(&'static str, SpaiType, SpaiStatus)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("/. ") {
        Some(("/.", SpaiType::Todo, SpaiStatus::Waiting))
    } else if trimmed.starts_with(". ") {
        Some((".", SpaiType::Todo, SpaiStatus::Todo))
    } else if trimmed.starts_with("/ ") {
        Some(("/", SpaiType::Todo, SpaiStatus::Working))
    } else if trimmed.starts_with("x ") || trimmed.starts_with("X ") {
        Some(("x", SpaiType::Todo, SpaiStatus::Done))
    } else if trimmed.starts_with("z ") || trimmed.starts_with("Z ") {
        Some(("z", SpaiType::Todo, SpaiStatus::Cancelled))
    } else if trimmed.starts_with("? ") {
        Some(("?", SpaiType::Idea, SpaiStatus::Idea))
    } else if trimmed.starts_with("- ") {
        Some(("-", SpaiType::Note, SpaiStatus::Note))
    } else {
        None
    }
}
