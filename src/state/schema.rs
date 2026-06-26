pub const CURRENT_VERSION: u32 = 1;

/// Maximum length of `MemorySection.recent`. Connecting to a host
/// pushes to the front; the tail is dropped past this bound.
pub const RECENT_MAX: usize = 20;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct SetupSection {
    #[serde(default)]
    pub include_check_done: bool,
    #[serde(default)]
    pub declined_include_injection: bool,
}

/// One entry in the recent-connections list. `ts` is Unix-epoch seconds,
/// which sorts numerically as recency descending without any parsing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RecentEntry {
    pub alias: String,
    pub ts: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct MemorySection {
    /// Legacy single-host pointer (pre-v0.6). Still read for migration
    /// (`recent[0]` defaults to this when `recent` is empty on load),
    /// but the v0.6 connect path writes only to `recent`. Slated for
    /// removal in v0.7.
    #[serde(default)]
    pub last_connected_alias: Option<String>,
    /// Most-recent-first connection history, bounded to `RECENT_MAX`.
    #[serde(default)]
    pub recent: Vec<RecentEntry>,
    /// User-pinned hosts. Order-preserving (insertion order); the
    /// picker sorts these to the top regardless of fuzzy score.
    #[serde(default)]
    pub favorites: Vec<String>,
    /// v0.12 G3: persisted sort-axis preference. Loaded into
    /// `App.sort_axis` on `App::new`; written back on every `S`
    /// press through `App::cycle_sort_axis`. `#[serde(default)]`
    /// means pre-v0.12 state.toml files load with the default
    /// (Alias), matching the v0.10 G5 starting axis.
    #[serde(default)]
    pub sort_axis: SortAxisPersisted,
}

/// v0.12 G3: state.toml-side representation of the sort axis. The
/// in-memory `SortAxis` enum lives in `app::mod` (it carries `Default`
/// behaviour and a cycle method tied to UI logic); keeping that crate
/// out of `state::schema` is what makes R-G6 hold. Conversion lives
/// in app (one match each way).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortAxisPersisted {
    #[default]
    Alias,
    Recent,
    Reachability,
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
