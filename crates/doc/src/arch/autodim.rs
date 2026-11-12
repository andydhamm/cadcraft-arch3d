use crate::Handle;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoRowKind {
    #[default]
    Openings,
    Walls,
    Overall,
}

impl AutoRowKind {
    pub fn key(self) -> &'static str {
        match self {
            AutoRowKind::Openings => "openings",
            AutoRowKind::Walls => "walls",
            AutoRowKind::Overall => "overall",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "openings" => Some(AutoRowKind::Openings),
            "walls" => Some(AutoRowKind::Walls),
            "overall" => Some(AutoRowKind::Overall),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoDimRow {
    pub side: String,
    pub kind: AutoRowKind,
    pub handles: Vec<Handle>,
    pub edited: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoDims {
    pub level: u32,
    pub rows: Vec<AutoRowKind>,
    pub generated: Vec<AutoDimRow>,
}

pub const MAX_AUTO_DIMS_PER_LEVEL: usize = 4096;
