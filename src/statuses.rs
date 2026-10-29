use crate::store::Status;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatusColor {
    Gray,
    Blue,
    Amber,
    Green,
    Purple,
    Rose,
}
impl StatusColor {
    pub const ALL: [Self; 6] = [
        Self::Gray,
        Self::Blue,
        Self::Amber,
        Self::Green,
        Self::Purple,
        Self::Rose,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gray => "gray",
            Self::Blue => "blue",
            Self::Amber => "amber",
            Self::Green => "green",
            Self::Purple => "purple",
            Self::Rose => "rose",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "blue" => Self::Blue,
            "amber" => Self::Amber,
            "green" => Self::Green,
            "purple" => Self::Purple,
            "rose" => Self::Rose,
            _ => Self::Gray,
        }
    }
    pub fn rgb(self, dark: bool) -> (f32, f32, f32) {
        let (r, g, b) = if dark {
            match self {
                Self::Gray => (185, 198, 215),
                Self::Blue => (134, 194, 255),
                Self::Amber => (255, 206, 126),
                Self::Green => (127, 220, 175),
                Self::Purple => (202, 173, 255),
                Self::Rose => (255, 166, 191),
            }
        } else {
            match self {
                Self::Gray => (77, 94, 113),
                Self::Blue => (30, 91, 155),
                Self::Amber => (137, 84, 9),
                Self::Green => (22, 110, 71),
                Self::Purple => (112, 66, 160),
                Self::Rose => (168, 55, 91),
            }
        };
        (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusDefinition {
    pub id: String,
    pub name: String,
    pub kind: Status,
    pub color: StatusColor,
    pub builtin: bool,
    pub version: i64,
}
impl StatusDefinition {
    pub fn builtin(kind: Status) -> Self {
        let (name, color) = match kind {
            Status::Pending => ("未完成", StatusColor::Gray),
            Status::InProgress => ("进行中", StatusColor::Blue),
            Status::Done => ("已完成", StatusColor::Green),
        };
        Self {
            id: kind.as_str().into(),
            name: name.into(),
            kind,
            color,
            builtin: true,
            version: 1,
        }
    }
}
pub fn custom_id(key: &str) -> Option<i64> {
    let id = key.strip_prefix("custom_")?.parse::<i64>().ok()?;
    (id > 0 && key == format!("custom_{id}")).then_some(id)
}
