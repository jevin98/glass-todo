use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    #[serde(other)]
    System,
}
impl Theme {
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: Theme,
    /// Background alpha only. Text and icons always remain fully opaque.
    pub opacity: u8,
    pub text_size: u8,
    pub radius: u8,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            opacity: 92,
            text_size: 14,
            radius: 28,
        }
    }
}
impl Appearance {
    pub fn normalized(mut self) -> Self {
        self.opacity = self.opacity.clamp(60, 100);
        self.text_size = self.text_size.clamp(12, 18);
        self.radius = self.radius.clamp(8, 32);
        self
    }
}
