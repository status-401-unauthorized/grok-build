//! Scrollback entries for the subagent lifecycle.
//!
//! Similar to BgTaskBlock: collapsed by default, an animated bullet while running, a colored bullet when done.
//! Expand to show the child session ID. Enter / Ctrl-F opens the subagent view.
//!
//! Two modes:
//! - **Blocking** (sync): one `Started` block; it blinks while running and turns green/red when done. Text: `Subagent "description"`
//! - **Background** (async): the `Started` block stays forever (turns gray) and a separate `Completed`/`Failed` block is added when done.
//!   Started text: `Subagent started: "description"`
//!   Completed text: `Subagent completed in 43s: "description"`

use std::time::Duration;

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::app::subagent::{format_session_id_line, format_subagent_meta};
use crate::appearance::AppearanceConfig;
use crate::render::color::blend_color;
use crate::render::line_utils::truncate_str;
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{AccentStyle, BlockContext, BlockLine, BlockOutput, DisplayMode};
use crate::theme::Theme;
use crate::util::format_duration;

/// What kind of subagent lifecycle event this block represents.
#[derive(Debug, Clone)]
pub enum SubagentBlockKind {
    /// Subagent is running (or was running; `finish_running` stops animation).
    Started,
    /// Subagent completed successfully.
    Completed { elapsed: Duration },
    /// Subagent failed.
    Failed {
        elapsed: Duration,
        error: Option<String>,
    },
    /// Subagent was cancelled.
    Cancelled { elapsed: Duration },
}

/// Collapsed by default; expand to show the child session ID.
/// Groupable and selectable. Enter / Ctrl-F opens the subagent view.
#[derive(Debug, Clone)]
pub struct SubagentBlock {
    /// Human-readable description of the task.
    pub description: String,
    /// Child session ID (for opening the subagent view).
    pub child_session_id: String,
    /// Subagent type (e.g. "general-purpose", "explore").
    pub subagent_type: String,
    /// Named persona applied to this subagent, if any.
    pub persona: Option<String>,
    /// Role that supplied defaults for this subagent, if any.
    pub role: Option<String>,
    /// Effective model ID used by the subagent, if available.
    pub model: Option<String>,
    /// Whether the subagent was launched in background mode.
    pub is_background: bool,
    /// Lifecycle kind.
    pub kind: SubagentBlockKind,
    /// Live activity label from the child session's turn tracker. The user sees interactive progress without opening
    /// the subagent view.
    pub activity_label: Option<String>,
}

impl SubagentBlock {
    /// Create a "Subagent started" block (for both sync and async).
    pub fn started(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        subagent_type: impl Into<String>,
        persona: Option<String>,
        role: Option<String>,
        model: Option<String>,
        is_background: bool,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: subagent_type.into(),
            persona,
            role,
            model,
            is_background,
            kind: SubagentBlockKind::Started,
            activity_label: None,
        }
    }

    /// Create a "Subagent completed" block (background mode only).
    pub fn completed(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        elapsed: Duration,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: String::new(),
            persona: None,
            role: None,
            model: None,
            is_background: true,
            kind: SubagentBlockKind::Completed { elapsed },
            activity_label: None,
        }
    }

    /// Create a "Subagent failed" block (background mode only).
    pub fn failed(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        elapsed: Duration,
        error: Option<String>,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: String::new(),
            persona: None,
            role: None,
            model: None,
            is_background: true,
            kind: SubagentBlockKind::Failed { elapsed, error },
            activity_label: None,
        }
    }

    /// Create a "Subagent cancelled" block (background mode only).
    pub fn cancelled(
        description: impl Into<String>,
        child_session_id: impl Into<String>,
        elapsed: Duration,
    ) -> Self {
        Self {
            description: description.into(),
            child_session_id: child_session_id.into(),
            subagent_type: String::new(),
            persona: None,
            role: None,
            model: None,
            is_background: true,
            kind: SubagentBlockKind::Cancelled { elapsed },
            activity_label: None,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.kind, SubagentBlockKind::Started)
    }

    fn session_id_body_line(&self, muted: ratatui::style::Style) -> BlockLine {
        BlockLine::styled(Line::from(Span::styled(
            format_session_id_line(&self.child_session_id),
            muted,
        )))
        .with_selection_range(Some(0))
        .with_selection_text(Some(self.child_session_id.clone()))
    }

    /// ` · Session ID: {id}` for the collapsed one-line header.
    /// Verb-group expansion ("Ran 2 subagents") reveals these collapsed members,
    /// so the id has to live on this line — not only on the individually expanded body.
    fn session_id_suffix(&self) -> String {
        if self.child_session_id.is_empty() {
            String::new()
        } else {
            format!(" \u{00b7} {}", format_session_id_line(&self.child_session_id))
        }
    }
}

/// Truncate description and wrap in quotes for display.
fn quoted_desc(desc: &str, max_width: usize) -> String {
    // Reserve 2 chars for quotes
    if max_width <= 2 {
        return "\u{201C}\u{2026}\u{201D}".to_string(); // "…"
    }
    let inner = truncate_str(desc, max_width - 2);
    format!("\u{201C}{inner}\u{201D}")
}

impl BlockContent for SubagentBlock {
    fn output(&self, ctx: &BlockContext) -> BlockOutput {
        let theme = Theme::current();
        // When selected, lift only the bold "Subagent" label to `text_primary` so it reads as undimmed
        // This mirrors `read.rs` and `search.rs`, which bump only the label and leave the rest at `muted`
        // The detail text (verb, description, meta) stays muted in every state
        let bold = if ctx.is_selected {
            theme.primary().add_modifier(Modifier::BOLD)
        } else {
            theme.muted().add_modifier(Modifier::BOLD)
        };
        let muted = theme.muted();
        let w = ctx.width as usize;

        let sid_suffix = self.session_id_suffix();
        let line = match (&self.kind, self.is_background) {
            (SubagentBlockKind::Started, bg) => {
                let verb = if bg { "started: " } else { "running: " };
                let activity_suffix: String = self
                    .activity_label
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|a| format!(" \u{00b7} {a}"))
                    .unwrap_or_default();
                let meta = format_subagent_meta(
                    self.persona.as_deref(),
                    self.role.as_deref(),
                    self.model.as_deref(),
                );
                // "Subagent running: " / "Subagent started: " = 18 chars
                let overhead = 18 + meta.width() + activity_suffix.width() + sid_suffix.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(overhead));
                let mut spans = vec![
                    Span::styled("Subagent ", bold),
                    Span::styled(verb, muted),
                    Span::styled(desc, muted),
                ];
                if !activity_suffix.is_empty() {
                    spans.push(Span::styled(activity_suffix, muted));
                }
                spans.push(Span::styled(meta, muted));
                if !sid_suffix.is_empty() {
                    spans.push(Span::styled(sid_suffix, muted));
                }
                Line::from(spans)
            }
            // Completed: Subagent completed in Xs: "description"
            (SubagentBlockKind::Completed { elapsed }, _) => {
                let time_str = format_duration(*elapsed);
                // "Subagent completed in Xs: " = 26 + time_str.len()
                let prefix_len = 26 + time_str.len() + sid_suffix.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(prefix_len));
                let mut spans = vec![
                    Span::styled("Subagent ", bold),
                    Span::styled(format!("completed in {time_str}: "), muted),
                    Span::styled(desc, muted),
                ];
                if !sid_suffix.is_empty() {
                    spans.push(Span::styled(sid_suffix, muted));
                }
                Line::from(spans)
            }
            // Failed: Subagent failed in Xs: "description"
            (SubagentBlockKind::Failed { elapsed, error }, _) => {
                let time_str = format_duration(*elapsed);
                let detail = error
                    .as_deref()
                    .map(|e| format!(" ({e})"))
                    .unwrap_or_default();
                let prefix_len = 21 + time_str.len() + detail.len() + sid_suffix.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(prefix_len));
                let mut spans = vec![
                    Span::styled("Subagent ", bold),
                    Span::styled(format!("failed in {time_str}{detail}: "), muted),
                    Span::styled(desc, muted),
                ];
                if !sid_suffix.is_empty() {
                    spans.push(Span::styled(sid_suffix, muted));
                }
                Line::from(spans)
            }
            // Cancelled: Subagent cancelled in Xs: "description"
            (SubagentBlockKind::Cancelled { elapsed }, _) => {
                let time_str = format_duration(*elapsed);
                // "Subagent cancelled in Xs: " = 26 + time_str.len()
                let prefix_len = 26 + time_str.len() + sid_suffix.width();
                let desc = quoted_desc(&self.description, w.saturating_sub(prefix_len));
                let mut spans = vec![
                    Span::styled("Subagent ", bold),
                    Span::styled(format!("cancelled in {time_str}: "), muted),
                    Span::styled(desc, muted),
                ];
                if !sid_suffix.is_empty() {
                    spans.push(Span::styled(sid_suffix, muted));
                }
                Line::from(spans)
            }
        };

        let mut lines = vec![line.into()];
        if ctx.mode != DisplayMode::Collapsed && !self.child_session_id.is_empty() {
            lines.push(BlockLine::separator(Line::from("")));
            lines.push(self.session_id_body_line(muted));
        }
        BlockOutput { lines }
    }

    fn accent(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        let theme = Theme::current();
        match &self.kind {
            SubagentBlockKind::Started if ctx.is_running => {
                Some(AccentStyle::static_color(theme.accent_running))
            }
            _ => None,
        }
    }

    fn bullet(&self, ctx: &BlockContext) -> Option<AccentStyle> {
        let theme = Theme::current();
        match &self.kind {
            SubagentBlockKind::Started => {
                if ctx.is_running {
                    let dim = ctx.appearance.scrollback.display.dim_accent;
                    let dimmed = blend_color(theme.bg_base, theme.accent_running, dim)
                        .unwrap_or(theme.accent_running);
                    Some(AccentStyle::animated(dimmed))
                } else {
                    // Finished: gray bullet (same as bg task "started" after completion)
                    None
                }
            }
            SubagentBlockKind::Completed { .. } => {
                Some(AccentStyle::static_color(theme.accent_success))
            }
            SubagentBlockKind::Failed { .. } | SubagentBlockKind::Cancelled { .. } => {
                Some(AccentStyle::static_color(theme.accent_error))
            }
        }
    }

    fn has_vpad_for(&self, _appearance: &AppearanceConfig) -> bool {
        false
    }

    fn has_raw_mode(&self) -> bool {
        false
    }

    fn is_foldable(&self) -> bool {
        !self.child_session_id.is_empty()
    }

    fn default_display_mode(&self) -> DisplayMode {
        DisplayMode::Collapsed
    }

    fn is_selectable(&self) -> bool {
        true
    }

    fn has_bullet(&self, _ctx: &BlockContext) -> bool {
        true
    }

    fn is_groupable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::AppearanceConfig;

    fn ctx(mode: DisplayMode) -> BlockContext {
        BlockContext {
            mode,
            is_running: false,
            width: 80,
            raw: false,
            max_lines: None,
            appearance: AppearanceConfig::default(),
            is_selected: false,
            cwd: None,
        }
    }

    fn line_text(line: &BlockLine) -> String {
        line.content
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn foldable_when_child_session_id_present() {
        let block = SubagentBlock::started(
            "scan src",
            "child-sess-1",
            "explore",
            None,
            None,
            None,
            false,
        );
        assert!(block.is_foldable());
        assert_eq!(block.default_display_mode(), DisplayMode::Collapsed);
    }

    #[test]
    fn not_foldable_without_child_session_id() {
        let block = SubagentBlock::started("scan src", "", "explore", None, None, None, false);
        assert!(!block.is_foldable());
    }

    #[test]
    fn collapsed_header_includes_session_id() {
        let block = SubagentBlock::started(
            "scan src",
            "child-sess-1",
            "explore",
            None,
            None,
            None,
            false,
        );
        let out = block.output(&ctx(DisplayMode::Collapsed));
        assert_eq!(out.lines.len(), 1);
        let text = line_text(&out.lines[0]);
        assert!(text.contains("Subagent"), "{text}");
        assert!(text.contains("scan src"), "{text}");
        assert!(
            text.contains("Session ID: child-sess-1"),
            "verb-group expansion shows collapsed members: {text}"
        );
    }

    #[test]
    fn expanded_output_shows_session_id() {
        let block = SubagentBlock::started(
            "scan src",
            "child-sess-1",
            "explore",
            None,
            None,
            None,
            false,
        );
        let out = block.output(&ctx(DisplayMode::Expanded));
        assert!(out.lines.len() >= 3, "header + gap + session id");
        let body = line_text(out.lines.last().unwrap());
        assert_eq!(body, "Session ID: child-sess-1");
        assert_eq!(
            out.lines.last().unwrap().selection_text.as_deref(),
            Some("child-sess-1")
        );
    }

    #[test]
    fn completed_expanded_still_shows_session_id() {
        let block = SubagentBlock::completed(
            "scan src",
            "child-sess-9",
            std::time::Duration::from_secs(2),
        );
        let out = block.output(&ctx(DisplayMode::Expanded));
        let body = line_text(out.lines.last().unwrap());
        assert_eq!(body, "Session ID: child-sess-9");
    }
}
