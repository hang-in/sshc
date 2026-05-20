//! Filter + navigation methods on `impl super::App`.

use super::App;

impl App {
    /// Re-compute `filtered` based on the current `filter_query`.
    ///
    /// - `@<needle>` → tag-only filter. `@` alone lists every host that has any tag.
    /// - bare query → nucleo fuzzy match against alias/hostname; tag substring is a
    ///   fallback when the fuzzy score is 0.
    ///
    /// Sort order (highest priority first):
    ///   1. favorited hosts (`state.memory.favorites`) float to the top
    ///   2. fuzzy / tag-substring score, descending
    pub(super) fn apply_filter(&mut self) {
        let query = self.filter_query.clone();
        // Snapshot the favorites list once so the sort comparator doesn't
        // need to re-borrow `self`.
        let favorites: std::collections::HashSet<String> =
            self.state.memory.favorites.iter().cloned().collect();
        let is_fav = |idx: usize| favorites.contains(&self.hosts[idx].alias);

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
            indices.sort_by(|&a, &b| is_fav(b).cmp(&is_fav(a)));
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
            scored.sort_by(|a, b| is_fav(b.0).cmp(&is_fav(a.0)).then(b.1.cmp(&a.1)));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }

        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }
}
