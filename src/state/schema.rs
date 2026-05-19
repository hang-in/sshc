pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct SetupSection {
    #[serde(default)]
    pub include_check_done: bool,
    #[serde(default)]
    pub declined_include_injection: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct MemorySection {
    #[serde(default)]
    pub last_connected_alias: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct State {
    pub version: u32,
    #[serde(default)]
    pub setup: SetupSection,
    #[serde(default)]
    pub memory: MemorySection,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            setup: SetupSection::default(),
            memory: MemorySection::default(),
        }
    }
}
