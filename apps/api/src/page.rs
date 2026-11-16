//! Cursor pagination: `?limit=50&cursor=<id>`. IDs are UUIDv7, so ordering by ID is ordering by
//! creation time, and the cursor is the last ID of the previous page.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

pub const DEFAULT_LIMIT: i64 = 50;
pub const MAX_LIMIT: i64 = 200;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

pub struct PageArgs {
    pub limit: i64,
    /// Rows after this ID; the nil UUID means from the start.
    pub after: Uuid,
}

impl PageQuery {
    pub fn args(&self) -> Result<PageArgs, ApiError> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(ApiError::invalid("limit", &format!("limit must be between 1 and {MAX_LIMIT}.")));
        }
        let after = match self.cursor.as_deref() {
            None | Some("") => Uuid::nil(),
            Some(c) => Uuid::parse_str(c).map_err(|_| ApiError::invalid("cursor", "cursor is not one this server gave out."))?,
        };
        Ok(PageArgs { limit, after })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// Fetch `limit + 1` rows, then call this: the extra row only tells whether there is a next page.
pub fn page<T>(mut rows: Vec<T>, limit: i64, id: impl Fn(&T) -> Uuid) -> Page<T> {
    let limit = usize::try_from(limit).unwrap_or(0);
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if more { rows.last().map(|r| id(r).to_string()) } else { None };
    Page { items: rows, next_cursor }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_and_cursors_are_checked() {
        assert_eq!(PageQuery::default().args().unwrap().limit, 50);
        assert!(PageQuery { limit: Some(0), cursor: None }.args().is_err());
        assert!(PageQuery { limit: Some(201), cursor: None }.args().is_err());
        assert!(PageQuery { limit: None, cursor: Some("x".into()) }.args().is_err());
    }

    #[test]
    fn next_cursor_only_when_there_is_more() {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::now_v7()).collect();
        let p = page(ids.clone(), 2, |u| *u);
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.next_cursor, Some(ids[1].to_string()));
        let p = page(ids, 3, |u| *u);
        assert_eq!(p.next_cursor, None);
    }
}
