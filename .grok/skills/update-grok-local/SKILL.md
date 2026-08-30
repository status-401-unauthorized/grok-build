---
name: update-grok-local
description: >
  Sync this fork’s main from the xAI upstream remote (detect by URL, not
  remote name), resolve and analyze merge conflicts for fork-specific
  changes, rebuild xai-grok-pager only when upstream commits were merged,
  and verify grok-local version. Use when the user runs /update-grok-local,
  says “update grok-local”, “sync upstream into fork”, “refresh local grok
  build”, or wants to pull monorepo main and rebuild the local binary.
metadata:
  short-description: "Sync xAI main → fork; rebuild only if commits merged"
---

# /update-grok-local — Sync upstream, preserve fork deltas, rebuild if needed

End-to-end workflow to pull the xAI upstream `main` into this fork’s `main`,
keep or adapt fork-only changes after conflict analysis, rebuild the local
binary **only when this run merged upstream commits**, confirm
`grok-local version` (against the just-built binary after a rebuild, or the
existing binary when the rebuild was skipped), then **review this skill**
for vital updates and show any suggestions to the user before closing.

## Preconditions (abort early if unmet)

Run from the **grok-build** repo root (the tree that contains
`crates/codegen/xai-grok-pager-bin`).

1. Detect remotes **by fetch URL**, not by name (`origin` / `upstream` /
   `fork` are swapped on some clones):
   - **Upstream** = remote whose URL contains `xai-org/grok-build`
   - **Fork** = the other `grok-build` remote (this user’s publish target)
   Abort if upstream cannot be identified.
2. Working tree should be clean enough to merge. If dirty:
   - Prefer stashing only if the user did not intentionally leave WIP.
   - If WIP looks intentional, **stop and ask** before discarding or stashing.
3. Preferred branch: local `main` tracking `$FORK_REMOTE/main`. If on another
   branch, tell the user and ask whether to switch to `main` or update the
   current branch instead.

Record before any mutation:

```bash
git remote -v
git status -sb
git rev-parse --abbrev-ref HEAD
git rev-parse --short HEAD

# Detect remotes by fetch URL. Names vary (this clone: upstream=xAI, origin=fork).
UPSTREAM_REMOTE=$(git remote -v | awk '/github.com[:/]xai-org\/grok-build/ && /fetch/ {print $1; exit}')
FORK_REMOTE=$(git remote -v | awk '!/github.com[:/]xai-org\/grok-build/ && /grok-build/ && /fetch/ {print $1; exit}')
echo "UPSTREAM_REMOTE=${UPSTREAM_REMOTE:-MISSING}  FORK_REMOTE=${FORK_REMOTE:-MISSING}"
# Abort if UPSTREAM_REMOTE is empty.

git rev-parse --short "${UPSTREAM_REMOTE}/main" 2>/dev/null || true
git rev-parse --short "${FORK_REMOTE}/main" 2>/dev/null || true
```

Use `$UPSTREAM_REMOTE` and `$FORK_REMOTE` for every later fetch, merge,
compare, and push.

## Remote / branch model

| Role | How to detect | Branch |
|------|---------------|--------|
| Upstream source of truth | URL contains `xai-org/grok-build` | `main` |
| Local integration branch | (local) | `main` |
| Publish target for this fork | the non-xAI `grok-build` remote | `main` |

Merge **upstream into local main**, then (only if user asks or skill is run
with push intent) update `$FORK_REMOTE/main`. Default of this skill:
**local merge; build only if this run merged upstream commits**; do **not**
`git push` without explicit user approval.

## Steps

### 1. Fetch upstream

```bash
git fetch "$UPSTREAM_REMOTE" main
# Optional but useful for comparison:
git fetch "$FORK_REMOTE" main
```

Show how far behind the xAI tip:

```bash
git log --oneline --left-right --cherry-pick HEAD..."$UPSTREAM_REMOTE/main" | head -40
git rev-list --left-right --count HEAD..."$UPSTREAM_REMOTE/main"
```

If already up to date with `$UPSTREAM_REMOTE/main` (0 commits to merge), skip
merge, conflict, analysis, **and build**. Jump to **Step 7 (version report)**
for the existing binary, then **Step 8**. Do **not** rebuild unless the user
explicitly asked to rebuild anyway.

A previously fetched but unmerged tip still has commits to merge — do not treat
“fetch did not move the remote-tracking ref” as “already up to date.”

### 2. Merge upstream main into local main

Ensure on the integration branch (default `main`):

```bash
git checkout main
PRE_MERGE_HEAD=$(git rev-parse HEAD)
echo "PRE_MERGE_HEAD=$PRE_MERGE_HEAD"
git merge "$UPSTREAM_REMOTE/main"
```

Commit message style used in this repo when wrapping merges:

```text
Merge <upstream-remote>/main: sync monorepo into fork; <brief note of preserved fork deltas>
```

If `git merge` reports “Already up to date” (0 commits merged), skip
Steps 3–6 and jump to Step 7, then Step 8. Do **not** rebuild.

If the merge completes cleanly **and brought in upstream commits**, note
“no conflicts” and continue to Step 4 with a light fork-delta review
(Step 4 still runs — use the pre-merge fork tip vs merge-base to list
unique fork commits).

### 3. Resolve conflicts

If merge stops with conflicts:

```bash
git status
git diff --name-only --diff-filter=U
```

For each conflicted file:

1. Read both sides (`git show :2:path`, `git show :3:path`, and the working
   tree conflict markers).
2. Prefer **preserving intentional fork behavior** unless upstream clearly
   supersedes it (see Step 4 criteria).
3. Resolve markers completely — no leftover `<<<<<<<`, `=======`, `>>>>>>>`.
4. `git add` each resolved path.

Do **not** `git merge --abort` unless the user asks or resolution is
impossible without guidance. If stuck after serious analysis, present options
and ask.

### 4. Analyze conflicts / fork-only changes

This step is mandatory even when Git auto-merged: fork intent must still hold.

**Identify fork-only work** (commits on local/fork not in upstream):

```bash
# Commits on HEAD that are not on the xAI tip (before merge: use pre-merge tip)
git log --oneline "$UPSTREAM_REMOTE/main"..HEAD   # or: merge-base..fork-tip if mid-merge
```

Also use conflict files + recent merge commit messages (e.g. “keep pager error
UI”) as hints of deliberate fork deltas.

For **each** conflicted path or non-trivial fork delta, write a short analysis
(to the user, not necessarily a file):

| Question | How to decide |
|----------|----------------|
| **(a) Still needed?** | Is the fork change still a product/UX goal? Did upstream land an equivalent or better fix? Search upstream diff for related symbols/tests. |
| **(b) Still works as before?** | After resolution, do APIs/types/call sites still match? Re-read callers, tests, and any UI render paths touched by both sides. |

Outcomes per delta:

- **Keep as-is** — re-apply or retain fork side; cite why.
- **Adapt** — upstream moved; port fork intent onto new structure (Step 5).
- **Drop** — upstream fully supersedes; document why.

Known historical fork themes (update if superseded) — treat as high-priority
intent unless analysis shows upstream absorbed them:

1. **Pager tool-error UI** — copyable tool errors, hide paths in collapsed
   headers, and **failed Edit expansion**. A failed `search_replace` must
   not collapse to a one-line label only (`No matches found` / `Invalid
   input`). Collapsed: short reason suffix on the header (same pattern as
   Read). Expanded: detailed tool reason (nearest-match / confusable /
   invalid-input explanation) plus **Searched for:** `old_string` and
   **Replacement:** `new_string` (snake_case or camelCase). Built in
   `extract_edit_error` / `format_edit_error` (`acp/tracker.rs`); Edit
   header suffix in `scrollback/blocks/tool/edit.rs`. Tests:
   `extract_edit_error_*`, `failed_edit_block_stores_enriched_error`,
   `collapsed_failure_shows_basename_and_error_reason`,
   `expanded_failure_header_is_path_only_body_has_full_error`. Primary
   paths: `scrollback/blocks/tool/*`, `scrollback/block.rs`,
   `acp/tracker.rs`, `acp/tracker_tests.rs`.
2. **Plugin hooks at spawn** — merge enabled+trusted plugin hooks into the
   session `HookRegistry` at session start (not only on mid-session
   ReloadHooks / ReloadPlugins). Primary path:
   `crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs`
   (look for `append_specs` / “merged plugin hooks into session registry at
   spawn”). Upstream still wires plugin hooks mainly on reload; preserve the
   spawn-time merge unless upstream lands an equivalent.
3. **Session turn index UI** — show the 0-based `/session-info` `Turn: N`
   index on (a) plain user-prompt **scrollback bubbles** and (b) the
   **composer** input prefix (`❯ Turn N …`). Plain prompts only (not bash,
   cron, or interjections). Key pieces:
   - Bubble render: `scrollback/blocks/user.rs` (`prompt_index` → `Turn {n} `)
   - Optimistic stamp on local drain: `app/dispatch/queue.rs`
   - Shell stamp preferred on echo: `acp/tracker.rs` (always apply
     `promptIndex` meta, not only when `None`). Fork-only tracker tests
     live in `acp/tracker_tests.rs` (e.g. `echo_prompt_index_backfill_*`).
   - Next index for composer: `AgentView::next_session_turn_index` in
     `app/agent_view/session.rs` (max stamped `prompt_index` + 1, else plain
     user-prompt count)
   - Composer style/render: `views/prompt_widget/mod.rs` (`PromptStyle.turn_index`)
   - Full TUI wiring: `app/agent_view/render.rs`
   - Minimal mode wiring: `xai-grok-pager-minimal` `live.rs` / `overlay.rs`
     / `plan.rs` (chromeless feedback styles still set `turn_index: None`)
   - **`PromptStyle` field-list conflicts:** when upstream adds a new field
     next to fork-only `turn_index` (example: `placeholder_when_focused`),
     keep **both** `turn_index` and every new upstream field on the struct,
     `Default` / `inline()` helpers, and every full `PromptStyle { … }`
     literal. Never take a single side of a field-list conflict — upstream-only
     drops turn labels; HEAD-only fails to compile when a required field is
     missing.
   - **Echo-skip loop conflicts:** upstream’s `skip_next_user_echo` path in
     `acp/tracker.rs` still gates `promptIndex` on `block.prompt_index.is_none()`
     and may add sibling backfills in the same loop (example: `replay_ts` →
     `entry.created_at`). Keep fork always-apply `prompt_index` **and** every
     upstream sibling backfill. Never take a single side — upstream-only drops
     shell-authoritative turn stamps; HEAD-only drops replay timestamps (or
     whatever else upstream added next to the stamp).
4. **Windows proto-build / pager stack** — native Windows link of
   `xai-grok-pager-bin`. Three pieces, all required:
   - Skip the in-repo `bin/protoc` DotSlash wrapper (shebang + JSON, not a
     PE `MZ` image; `CreateProcess` fails with `ERROR_BAD_EXE_FORMAT`) and
     fall back to PATH / `$PROTOC`.
   - Write `protoc --dependency_out` / `--descriptor_set_out` to temp files
     (`/dev/stdout` and `/dev/null` are not valid Windows paths).
   - Raise the pager main-thread stack to 8 MiB (`/STACK:8388608` on MSVC,
     `-Wl,--stack,8388608` otherwise) so clap parsing does not overflow.
   Primary paths:
   - `crates/build/xai-proto-build/src/find_protoc.rs` (`is_pe_executable`)
   - `crates/build/xai-proto-build/src/lib.rs` (Windows temp-file deps,
     `makefile_dependency_paths`)
   - `crates/codegen/xai-grok-pager-bin/build.rs` (`CARGO_CFG_TARGET_OS`
     windows stack link-arg)
   If upstream changes protoc discovery, `--dependency_out`, or pager-bin
   `build.rs`, keep the Windows PE skip, temp-file deps, and 8 MiB stack
   **and** every new upstream path. Never take a single side.

**`tracker.rs` test-extract conflicts:** upstream owns unit tests in
`acp/tracker_tests.rs` (`#[cfg(test)]` + `#[path = "tracker_tests.rs"]
mod tests;`). If a conflict is inline `mod tests { … }` vs that path
attribute, **take the extract** and **port** any fork-only tests into
`tracker_tests.rs`. Do not keep the inline module — the next upstream
sync will re-conflict a multi-thousand-line test blob.

**Adjacent re-check (even with a clean merge / no fork-file conflicts):**
fork intent can break without a Git conflict on the fork files themselves.

**Pager tool-error UI** — if the upstream range touches clipboard delivery,
text selection, or tool-block render/copy paths, re-verify selection ranges
and copyable failure bodies. Watch at least:

```text
crates/codegen/xai-grok-pager-render/src/clipboard/
crates/codegen/xai-grok-pager/src/scrollback/block.rs
crates/codegen/xai-grok-pager/src/scrollback/blocks/tool/
crates/codegen/xai-grok-pager/src/acp/tracker.rs
crates/codegen/xai-grok-pager/src/acp/tracker_tests.rs
```

**Plugin hooks at spawn** — if the upstream range touches session spawn,
hook reload, or plugin registry snapshot application, re-verify the
spawn-time plugin `append_specs` path still exists after merge. Watch at
least:

```text
crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs
```

Also skim related reload/snapshot helpers when they appear in the upstream
diff (e.g. `reload_hooks_impl`, `apply_plugin_registry_snapshot`, or other
`acp_session_impl/*` hook/plugin wiring). Confirm post-merge working tree
still merges plugin file + inline hooks into `built_hook_registry` before
the session actor starts.

**Session turn index UI** — if the upstream range touches user-prompt
render, prompt_index stamping, queue drain paint, composer prefix layout,
or minimal live prompt style, re-verify both surfaces still show the
0-based session turn. Watch at least:

```text
crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs
crates/codegen/xai-grok-pager/src/views/prompt_widget/mod.rs
crates/codegen/xai-grok-pager/src/app/agent_view/session.rs
crates/codegen/xai-grok-pager/src/app/agent_view/render.rs
crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs
crates/codegen/xai-grok-pager/src/acp/tracker.rs
crates/codegen/xai-grok-pager/src/acp/tracker_tests.rs
crates/codegen/xai-grok-pager-minimal/src/live.rs
crates/codegen/xai-grok-pager-minimal/src/overlay.rs
crates/codegen/xai-grok-pager-minimal/src/plan.rs
```

Confirm post-merge: plain bubbles render `Turn {n}` when `prompt_index` is
set; composer shows next index via `PromptStyle.turn_index` +
`next_session_turn_index()`; bash/cron/interjections stay unlabeled;
minimal `prompt_style(..., turn_index)` still wired; every `PromptStyle`
literal still has both `turn_index` and any upstream-only fields (e.g.
`placeholder_when_focused`).

**Windows proto-build / pager stack** — if the upstream range touches
protoc discovery, proto-build dependency emission, or pager-bin `build.rs`,
re-verify the Windows PE skip, temp-file `--dependency_out`, and 8 MiB
stack link-arg still exist. Watch at least:

```text
crates/build/xai-proto-build/src/find_protoc.rs
crates/build/xai-proto-build/src/lib.rs
crates/codegen/xai-grok-pager-bin/build.rs
```

How to check (use the **commits being merged** — not
`PRE_MERGE_HEAD..$UPSTREAM_REMOTE/main` and not a pre-fetch tip):

`PRE_MERGE_HEAD..$UPSTREAM_REMOTE/main` is a tree comparison. When the fork
already diverges in `tool/*`, `tracker.rs`, `tracker_tests.rs`, `spawn.rs`,
turn-index UI paths, or Windows proto-build / pager-bin `build.rs`, those
paths show up as “changed” even if upstream never touched them this sync.
A pre-fetch tip equals the current xAI tip whenever those commits were
already fetched but not merged — that range is then empty.

Always diff merge-base → xAI tip:

```bash
UPSTREAM_TIP="$UPSTREAM_REMOTE/main"
UPSTREAM_BASE=$(git merge-base "$PRE_MERGE_HEAD" "$UPSTREAM_TIP")

# Did this upstream sync touch selection/copy-adjacent code?
git diff --name-only "${UPSTREAM_BASE}".."$UPSTREAM_TIP" -- \
  crates/codegen/xai-grok-pager-render/src/clipboard/ \
  crates/codegen/xai-grok-pager/src/scrollback/ \
  crates/codegen/xai-grok-pager/src/acp/

# Did this upstream sync touch session spawn / plugin-hook wiring?
git diff --name-only "${UPSTREAM_BASE}".."$UPSTREAM_TIP" -- \
  crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs \
  crates/codegen/xai-grok-shell/src/session/acp_session_impl/

# Did this upstream sync touch turn-index UI (bubble + composer)?
git diff --name-only "${UPSTREAM_BASE}".."$UPSTREAM_TIP" -- \
  crates/codegen/xai-grok-pager/src/scrollback/blocks/user.rs \
  crates/codegen/xai-grok-pager/src/views/prompt_widget/ \
  crates/codegen/xai-grok-pager/src/app/agent_view/ \
  crates/codegen/xai-grok-pager/src/app/dispatch/queue.rs \
  crates/codegen/xai-grok-pager-minimal/src/

# Did this upstream sync touch Windows proto-build / pager stack?
git diff --name-only "${UPSTREAM_BASE}".."$UPSTREAM_TIP" -- \
  crates/build/xai-proto-build/src/find_protoc.rs \
  crates/build/xai-proto-build/src/lib.rs \
  crates/codegen/xai-grok-pager-bin/build.rs
```

If any pager watch paths appear, skim the upstream diff for selection ranges,
`CopyDelivery` / toast wiring, and tool failure body handling; confirm
collapsed error suffixes and copyable error text still work (read call sites
or run focused tool-block / selection tests when practical). For Edit
failures, confirm `extract_edit_error` still keeps the short first-line
label **and** the expanded body (detail + Searched for / Replacement),
and that `edit.rs` still paints the collapsed reason suffix. Do not
accept an upstream-only short label that discards `old_string`.

If `spawn.rs` (or related hook/plugin helpers) appear, skim the upstream
diff and re-read the post-merge spawn hook-registry block; confirm plugin
hooks are still appended at spawn (not only on reload).

If turn-index paths appear, re-read bubble + composer wiring; confirm
`Turn {n}` still shows on plain prompts and the composer next-index still
matches `/session-info` semantics.

If proto-build or pager-bin `build.rs` appear, re-read the post-merge
Windows branches; confirm the DotSlash wrapper is still skipped, deps
still go to a temp file, and the 8 MiB stack link-arg is still emitted.

Note outcomes in the fork-analysis section of the completion report
(“adjacent re-check: pass / adapt needed” per theme, or “n/a — paths
untouched”).

### 5. Implement post-analysis adjustments

If Step 4 requires code changes beyond pure conflict resolution:

1. Implement the adapted keep/drop decisions.
2. Prefer small, reviewable edits in the same merge resolution window.
3. Run focused tests if available for touched areas (e.g. pager scrollback /
   tool-block tests). Do not block forever on full suite; note what was skipped.
4. Stage and include in the merge commit if still in progress, or make a
   follow-up commit on `main` with a clear message.

### 6. Build the local binary

**Skip the rebuild** when this run merged **zero** commits from upstream
(already up to date after fetch, or `git merge` reported “Already up to
date”). Do not run `cargo build`. Record
`Build: skipped — no upstream commits merged` and continue to Step 7
(report the existing binary) then Step 8.

Exception: rebuild anyway only if the user explicitly requested a rebuild
regardless of sync result (e.g. “rebuild anyway”, “refresh the binary”).

`grok-local` is expected to point at the **release** pager binary in this tree
(typically via `~/.bash_aliases`):

```bash
# Discover alias target if present (non-interactive shells may need to
# source ~/.bash_aliases first)
alias grok-local 2>/dev/null || true
type grok-local 2>/dev/null || true
```

Expected alias / default artifact:

```text
alias grok-local='HERDR_AGENT=grok $REPO/target/release/xai-grok-pager'
$REPO/target/release/xai-grok-pager
```

Build the **release** profile so the alias target is the binary just linked.
Do not `cargo build` (debug) and then verify via `grok-local` — that pair
leaves the release binary stale.

```bash
cargo build --release -p xai-grok-pager-bin
```

Use a long timeout (this crate is large; 15–30+ minutes can be normal on cold
builds). Prefer the workspace toolchain (`rust-toolchain.toml`).

If the build fails:

1. Fix compile errors caused by the merge/adaptation.
2. Rebuild until success.
3. Do not claim success without a successful link of
   `target/release/xai-grok-pager`.

If `grok-local` is missing from the current shell, use the explicit release
path (do not fall back to `target/debug/`):

```bash
HERDR_AGENT=grok "$REPO/target/release/xai-grok-pager" version
```

### 7. Verify version string

Source of truth for the marketing/semver version:

```text
crates/codegen/xai-grok-version/Cargo.toml  →  version = "X.Y.Z"
```

Also keep in lockstep (should already match after upstream sync):

```text
crates/codegen/xai-grok-pager/Cargo.toml
crates/codegen/xai-grok-pager-bin/Cargo.toml
```

The binary embeds at **compile time** (see `xai-grok-pager-bin/build.rs`):

```text
VERSION_WITH_COMMIT = "{CARGO_PKG_VERSION|GROK_VERSION} ({first 12 hex of git rev-parse HEAD})"
```

`git rev-parse --short HEAD` is a prefix of that 12-char stamp and is a valid
substring check. After a rebuild, prefer matching the 12-char stamp.

Channel suffix (`[stable]`, `[alpha]`, …) comes from runtime update config via
`xai_grok_update::channel_label()` — do not treat channel mismatch as a version
failure if semver + commit match.

Verification:

```bash
EXPECTED=$(grep -E '^version = ' crates/codegen/xai-grok-version/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
SHORT=$(git rev-parse --short HEAD)
COMMIT12=$(git rev-parse HEAD | cut -c1-12)
echo "Expected semver: $EXPECTED  stamp: $COMMIT12  --short: $SHORT"

# Prefer alias when available (source ~/.bash_aliases if needed).
# Fallback must be the release binary — same path the alias targets.
if alias grok-local >/dev/null 2>&1 || command -v grok-local >/dev/null 2>&1; then
  OUT=$(grok-local version 2>&1)
else
  OUT=$(HERDR_AGENT=grok ./target/release/xai-grok-pager version 2>&1)
fi
echo "$OUT"

# Pass criteria after a rebuild: output contains EXPECTED and COMMIT12
# (or SHORT as a prefix of COMMIT12). After a skipped rebuild, still print
# OUT vs EXPECTED/COMMIT12, but do not rebuild to “fix” a mismatch.
echo "$OUT" | grep -F "$EXPECTED" && echo "$OUT" | grep -F "$COMMIT12"
```

When the rebuild **ran**:

**Pass:** `grok-local version` (or equivalent path) shows `grok $EXPECTED ($COMMIT12) …`

**Fail if:**

- Still shows an older semver (stale binary / wrong path).
- Commit hash is an old build’s hash (binary not rebuilt after merge).
- Binary path is not the one just built (check alias →
  `target/release/xai-grok-pager`, not `target/debug/`).

On fail: confirm alias path, `ls -l` binary mtime, rebuild with
`cargo clean -p xai-grok-pager-bin` only if incremental link is wrong, then
re-run verification.

When the rebuild was **skipped** (no upstream commits merged): report the
existing `grok-local version` line vs `$EXPECTED ($COMMIT12)`. If they differ,
note the mismatch and that the binary was left as-is — do **not** rebuild
unless the user asks.

### 8. Review this skill (mandatory before closing)

After the build/version work is done (rebuild success, skipped rebuild, or
documented failure), re-read this file:

```text
.grok/skills/update-grok-local/SKILL.md
```

Compare the skill’s assumptions to **what just happened** in this run and to
any upstream changes that affect the procedure. Goal: catch vital drift so the
next `/update-grok-local` stays accurate.

**Check at least:**

| Area | Stale if… |
|------|-----------|
| Remotes / branches | URL-based detection no longer finds xAI vs this fork, or tracking model changed |
| Package / binary paths | `xai-grok-pager-bin`, artifact path, or `grok-local` wiring changed |
| Version sources | Semver crate, `build.rs` embed, or channel labeling changed |
| Fork themes | Upstream absorbed error-UI, plugin-hooks-at-spawn, session turn-index UI, or Windows proto-build / pager stack, or a new deliberate fork theme appeared |
| Adjacent watch paths | New surfaces matter for copy/selection/tool-error, plugin-hook spawn, turn-index UI, or Windows proto-build / pager stack (clipboard, scrollback, ACP, `spawn.rs`, composer, `xai-proto-build`, pager-bin `build.rs`, …) |
| Build / verify procedure | Toolchain, timeouts, env vars (`HERDR_AGENT`, `GROK_VERSION`), or pass criteria wrong |
| Safety / push policy | Process friction that should become an explicit rule |
| Operational gaps | Something non-obvious burned time this run and belongs in the skill |

**Output rules:**

1. Always show a **Skill review** section to the user (even when nothing is
   wrong).
2. If nothing vital needs changing, say so in one line:  
   `Skill review: no vital updates suggested.`
3. If something should change, list concrete suggestions — each with:
   - **What** is wrong or missing in the skill
   - **Why** (evidence from this run / upstream diff)
   - **Proposed edit** (brief; do not rewrite the whole skill in the report)
4. **Do not** edit `SKILL.md` in this step unless the user asks to apply
   suggestions. Present first; wait for approval.
5. Skip nitpicks (wording polish, redundant examples). Only flag **vital**
   procedure/accuracy gaps.

## Completion report (always print)

Summarize for the user:

1. **Sync:** pre/post SHAs (`HEAD`, `$UPSTREAM_REMOTE/main`), commits merged count.
2. **Conflicts:** files (or “none”), resolution summary.
3. **Fork analysis:** each delta → keep / adapt / drop + one-line rationale;
   include **adjacent re-check** results when upstream touched clipboard /
   selection / tool-block paths, session spawn / plugin-hook wiring,
   turn-index (bubble + composer) paths, and/or Windows proto-build /
   pager-bin `build.rs` (or “n/a — paths untouched” per theme).
4. **Code adjustments:** what was implemented after analysis (or “none”).
5. **Build:** success / fail / **skipped — no upstream commits merged**,
   package `xai-grok-pager-bin`, binary path + mtime (existing binary when
   skipped).
6. **Version check:** full `grok-local version` line vs expected
   `$EXPECTED ($COMMIT12)`. After a skip, report mismatch without rebuilding.
7. **Next steps (optional):** push to `$FORK_REMOTE/main` only if user wants:
   `git push "$FORK_REMOTE" main` (require confirmation — shared remote).
8. **Skill review:** “no vital updates” or numbered suggestions (Step 8). Never
   omit this section.

## Safety rules

- Never force-push to the upstream or fork remotes unless the user explicitly requests it.
- Never `git reset --hard` or discard uncommitted work without confirmation.
- Prefer resolving conflicts over aborting; abort only on user request or
  unrecoverable state.
- Do not skip Step 4 (analysis) when there were conflicts or unique fork
  commits.
- Do not rebuild when this run merged zero commits from upstream, unless the
  user explicitly asked to rebuild anyway.
- After a rebuild, do not claim version success without running the version
  command against the binary that was just built. After a skipped rebuild,
  report the existing binary’s version; do not claim it was just built.
- Do not skip Step 8 (skill review). Do not silently edit this skill; propose
  and wait for the user.

## Quick reference

```bash
# Full happy path (agent expands conflict/analysis + skill review as needed)
# UPSTREAM_REMOTE / FORK_REMOTE: detect by URL (see Preconditions)
git fetch "$UPSTREAM_REMOTE" main
git checkout main
PRE_MERGE_HEAD=$(git rev-parse HEAD)
git merge "$UPSTREAM_REMOTE/main"   # resolve + analyze if needed
# If 0 commits merged from upstream: skip cargo build; report existing version
# Adjacent re-check: git diff --name-only $(git merge-base $PRE_MERGE_HEAD $UPSTREAM_REMOTE/main)..$UPSTREAM_REMOTE/main -- <watch paths>
cargo build --release -p xai-grok-pager-bin   # only if this run merged upstream commits
grok-local version      # or HERDR_AGENT=grok ./target/release/xai-grok-pager version
# then: re-read this SKILL.md → report skill-review suggestions (or none)
```
