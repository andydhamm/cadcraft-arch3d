//! Filter history for library navigation.

use super::search::Query;

/// Tracks the history of library queries.
#[derive(Debug, Clone, Default)]
pub struct FilterHistory {
    entries: Vec<Query>,
    cursor: usize,
}

impl FilterHistory {
    /// Push a new query, dropping any forward history.
    pub fn push(&mut self, q: Query) {
        if self.entries.get(self.cursor) == Some(&q) {
            return;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(q);
        self.cursor = self.entries.len() - 1;
    }

    /// Move back in history.
    pub fn back(&mut self) -> Option<Query> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(self.entries[self.cursor].clone())
        } else {
            None
        }
    }

    /// Move forward in history.
    pub fn forward(&mut self) -> Option<Query> {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
            Some(self.entries[self.cursor].clone())
        } else {
            None
        }
    }

    /// Check if we can go back.
    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    /// Check if we can go forward.
    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// Get the current query.
    pub fn current(&self) -> Option<&Query> {
        self.entries.get(self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_back_forward_and_push_drops_forward() {
        let mut h = FilterHistory::default();
        assert!(!h.can_back());
        assert!(!h.can_forward());
        assert!(h.current().is_none());

        let q1 = Query { text: "a".into(), chips: vec![], ..Default::default() };
        h.push(q1.clone());
        assert_eq!(h.current(), Some(&q1));
        assert!(!h.can_back());
        assert!(!h.can_forward());

        let q2 = Query { text: "b".into(), chips: vec![], ..Default::default() };
        h.push(q2.clone());
        assert_eq!(h.current(), Some(&q2));
        assert!(h.can_back());
        assert!(!h.can_forward());

        let back = h.back();
        assert_eq!(back, Some(q1.clone()));
        assert_eq!(h.current(), Some(&q1));
        assert!(!h.can_back());
        assert!(h.can_forward());

        let q3 = Query { text: "c".into(), chips: vec![], ..Default::default() };
        h.push(q3.clone());
        assert_eq!(h.current(), Some(&q3));
        assert!(h.can_back());
        assert!(!h.can_forward());

        let back2 = h.back();
        assert_eq!(back2, Some(q1.clone()));
        let back3 = h.back();
        assert!(back3.is_none());
    }
}
