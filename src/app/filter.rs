//! Filter + navigation methods on `impl super::App`.

use super::{App, SortAxis};
use crate::probe::ProbeState;

impl App {
    /// Re-compute `filtered` based on the current `filter_query`.
    ///
    /// - `@<needle>` → tag-only filter. `@` alone lists every host that has any tag.
    /// - bare query → nucleo fuzzy match against alias/hostname; tag substring is a
    ///   fallback when the fuzzy score is 0.
    ///
    /// Sort order (highest priority first):
    ///   1. favorited hosts (`state.memory.favorites`) float to the top
    ///   2. fuzzy / tag-substring score, descending (when there's a query)
    ///   3. v0.10 G5 secondary axis — alias / recent / reachability — when
    ///      no fuzzy query is active
    pub(super) fn apply_filter(&mut self) {
        let query = self.filter_query.clone();
        // Snapshot the favorites list once so the sort comparator doesn't
        // need to re-borrow `self`.
        let favorites: std::collections::HashSet<String> =
            self.state.memory.favorites.iter().cloned().collect();
        let recency: std::collections::HashMap<String, u64> = self
            .state
            .memory
            .recent
            .iter()
            .map(|e| (e.alias.clone(), e.ts))
            .collect();
        let is_fav = |idx: usize| favorites.contains(&self.hosts[idx].alias);
        let ts_of = |idx: usize| recency.get(&self.hosts[idx].alias).copied().unwrap_or(0);
        let probe_rank = |idx: usize| match self.probe_states.get(idx).copied() {
            Some(ProbeState::Open) => 0u8,
            Some(ProbeState::InFlight) => 1,
            Some(ProbeState::Unknown) | None => 2,
            Some(ProbeState::Failed) => 3,
        };
        let axis = self.sort_axis;
        let alias_of = |idx: usize| self.hosts[idx].alias.to_lowercase();
        let axis_cmp = |a: usize, b: usize| match axis {
            SortAxis::AliasAlpha => alias_of(a).cmp(&alias_of(b)),
            SortAxis::RecentDesc => ts_of(b).cmp(&ts_of(a)),
            SortAxis::ProbeStateOpenFirst => probe_rank(a).cmp(&probe_rank(b)),
        };

        if let Some(tag_query) = query.strip_prefix('@') {
            let needle = tag_query.trim().to_lowercase();
            let mut indices: Vec<usize> = self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, h)| {
                    if needle.is_empty() {
                        !h.tags.is_empty()
                    } else {
                        h.tags.iter().any(|t| t.contains(&needle))
                    }
                })
                .map(|(i, _)| i)
                .collect();
            indices.sort_by(|&a, &b| is_fav(b).cmp(&is_fav(a)).then_with(|| axis_cmp(a, b)));
            self.filtered = indices;
        } else {
            let needle = query.to_lowercase();
            let mut scored: Vec<(usize, u32)> = self
                .hosts
                .iter()
                .enumerate()
                .filter_map(|(i, host)| {
                    let score = host.fuzzy_score(&query, &mut self.matcher);
                    let tag_match =
                        !needle.is_empty() && host.tags.iter().any(|t| t.contains(&needle));
                    let best = if tag_match { score.max(1) } else { score };
                    if best > 0 {
                        Some((i, best))
                    } else {
                        None
                    }
                })
                .collect();
            // With a non-empty fuzzy query the score is the user's
            // strongest signal; axis is a tie-breaker.
            // With no query, the score is 1 for every host (the empty
            // fuzzy match), so axis dominates exactly as expected.
            scored.sort_by(|a, b| {
                is_fav(b.0)
                    .cmp(&is_fav(a.0))
                    .then(b.1.cmp(&a.1))
                    .then_with(|| axis_cmp(a.0, b.0))
            });
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }

        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }
}
