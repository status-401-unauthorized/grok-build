//! Pin the Edit path header while a tall diff is scrolled.
//!
//! User-prompt sticky headers ([`super::sticky`]) mark conversation turns.
//! This is in-block: when an expanded Edit is top-clipped past its
//! `"Edit path/to/file"` line, that line stays at the top of the remaining
//! visible area so the file path does not disappear. The header is pushed
//! off with the block once there is no longer room for the header plus at
//! least one body row (CSS `position: sticky` containing-block rule).

use crate::appearance::AppearanceConfig;
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::tool::{TOOL_HEADER_RANGE, ToolCallBlock};
use crate::scrollback::types::{BlockLine, Selectable};

/// Paint plan for an Edit block that may have its path header pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickyEditHeaderPlan {
    /// Leading path-header rows in the cached output.
    pub header_rows: usize,
    /// First output line to paint after any pinned header.
    pub body_start: usize,
    /// When true, paint `output[0..header_rows]` at the top of the visible area.
    pub pinned: bool,
}

impl StickyEditHeaderPlan {
    /// No pin: paint from `content_skip` as usual.
    pub fn passthrough(content_skip: usize) -> Self {
        Self {
            header_rows: 0,
            body_start: content_skip,
            pinned: false,
        }
    }

    /// Pin the path header when it would be fully clipped and the remaining
    /// visible height still has room for the header plus at least one body row.
    pub fn plan(
        lines: &[BlockLine],
        content_skip: usize,
        visible_rows: u16,
        enabled: bool,
    ) -> Self {
        let header_rows = sticky_edit_header_row_count(lines);
        let pinned = enabled
            && header_rows > 0
            && content_skip >= header_rows
            && (visible_rows as usize) > header_rows;
        Self {
            header_rows,
            body_start: content_skip,
            pinned,
        }
    }

    /// Plan for this entry, or passthrough when the block is not an Edit
    /// with sticky headers enabled.
    pub fn for_entry(
        block: &RenderBlock,
        appearance: &AppearanceConfig,
        lines: &[BlockLine],
        content_skip: usize,
        visible_rows: u16,
    ) -> Self {
        if is_sticky_edit_candidate(block, appearance) {
            Self::plan(lines, content_skip, visible_rows, true)
        } else {
            Self::passthrough(content_skip)
        }
    }

    /// Number of leading rows occupied by a pinned header (0 if not pinned).
    pub fn pinned_rows(&self) -> usize {
        if self.pinned { self.header_rows } else { 0 }
    }

    /// Visible output line indices in paint order, with a row offset from
    /// the first visible content row.
    pub fn visible_lines(
        &self,
        total_lines: usize,
        max_rows: u16,
    ) -> impl Iterator<Item = (usize, u16)> + '_ {
        let pinned = self.pinned_rows();
        let body = self.body_start;
        let max = max_rows as usize;
        (0..pinned)
            .chain(body..total_lines)
            .take(max)
            .enumerate()
            .map(|(offset, idx)| (idx, offset as u16))
    }
}

/// Leading path-header rows of an Edit block's cached output.
///
/// Stops before the blank separator (and any error body that shares
/// [`TOOL_HEADER_RANGE`] after it) so only the `"Edit path"` line(s) pin.
pub fn sticky_edit_header_row_count(lines: &[BlockLine]) -> usize {
    lines
        .iter()
        .take_while(|line| {
            line.selection_range == Some(TOOL_HEADER_RANGE)
                && !matches!(line.selectable, Selectable::None)
        })
        .count()
}

/// Whether this block should consider pinning its path header.
pub fn is_sticky_edit_candidate(block: &RenderBlock, appearance: &AppearanceConfig) -> bool {
    appearance.scrollback.blocks.edit.sticky_header
        && matches!(block, RenderBlock::ToolCall(ToolCallBlock::Edit(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrollback::types::BlockLine;
    use ratatui::text::Line;

    fn header_line() -> BlockLine {
        BlockLine {
            content: Line::from("Edit src/main.rs"),
            selectable: Selectable::All,
            selection_range: Some(TOOL_HEADER_RANGE),
            ..Default::default()
        }
    }

    fn separator() -> BlockLine {
        BlockLine::separator(Line::from(""))
    }

    fn body_line(text: &str) -> BlockLine {
        BlockLine {
            content: Line::from(text.to_string()),
            selectable: Selectable::All,
            selection_range: Some(1),
            ..Default::default()
        }
    }

    fn sample_lines() -> Vec<BlockLine> {
        vec![
            header_line(),
            separator(),
            body_line("  10  let x = 1;"),
            body_line("  11  let y = 2;"),
            body_line("  12  let z = 3;"),
        ]
    }

    #[test]
    fn header_row_count_stops_at_separator() {
        assert_eq!(sticky_edit_header_row_count(&sample_lines()), 1);
    }

    #[test]
    fn header_row_count_includes_wrapped_path() {
        let mut lines = sample_lines();
        lines.insert(
            1,
            BlockLine {
                content: Line::from("    very/long/continuation.rs"),
                selectable: Selectable::All,
                selection_range: Some(TOOL_HEADER_RANGE),
                ..Default::default()
            },
        );
        assert_eq!(sticky_edit_header_row_count(&lines), 2);
    }

    #[test]
    fn header_row_count_ignores_error_body_after_separator() {
        let lines = vec![
            header_line(),
            separator(),
            BlockLine {
                content: Line::from("No matches found"),
                selectable: Selectable::All,
                selection_range: Some(TOOL_HEADER_RANGE),
                ..Default::default()
            },
        ];
        assert_eq!(sticky_edit_header_row_count(&lines), 1);
    }

    #[test]
    fn no_pin_when_header_still_visible() {
        let plan = StickyEditHeaderPlan::plan(&sample_lines(), 0, 10, true);
        assert!(!plan.pinned);
        assert_eq!(
            plan.visible_lines(5, 10).collect::<Vec<_>>(),
            vec![(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)]
        );
    }

    #[test]
    fn pins_when_header_would_be_clipped() {
        // skip past header + separator (content_skip = 2); 10 visible rows
        let plan = StickyEditHeaderPlan::plan(&sample_lines(), 2, 10, true);
        assert!(plan.pinned);
        assert_eq!(plan.pinned_rows(), 1);
        assert_eq!(
            plan.visible_lines(5, 10).collect::<Vec<_>>(),
            vec![(0, 0), (2, 1), (3, 2), (4, 3)]
        );
    }

    #[test]
    fn no_pin_when_remaining_height_cannot_fit_header_and_body() {
        let plan = StickyEditHeaderPlan::plan(&sample_lines(), 4, 1, true);
        assert!(!plan.pinned);
        assert_eq!(plan.visible_lines(5, 1).collect::<Vec<_>>(), vec![(4, 0)]);
    }

    #[test]
    fn no_pin_when_disabled() {
        let plan = StickyEditHeaderPlan::plan(&sample_lines(), 2, 10, false);
        assert!(!plan.pinned);
        assert_eq!(
            plan.visible_lines(5, 10).collect::<Vec<_>>(),
            vec![(2, 0), (3, 1), (4, 2)]
        );
    }

    #[test]
    fn passthrough_matches_linear_skip() {
        let plan = StickyEditHeaderPlan::passthrough(3);
        assert!(!plan.pinned);
        assert_eq!(
            plan.visible_lines(5, 10).collect::<Vec<_>>(),
            vec![(3, 0), (4, 1)]
        );
    }
}
