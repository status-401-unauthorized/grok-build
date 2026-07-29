---
name: update-grok-local
description: >
  Sync this fork’s main from upstream (origin/main), resolve and analyze merge
  conflicts for fork-specific changes, rebuild xai-grok-pager, and verify
  grok-local version matches the source version. Use when the user runs
  /update-grok-local, says “update grok-local”, “sync upstream into fork”,
  “refresh local grok build”, or wants to pull monorepo main and rebuild the
  local binary.
metadata:
  short-description: "Sync origin/main → fork, rebuild grok-local"
---

# /update-grok-local — Sync upstream, preserve fork deltas, rebuild

End-to-end workflow to pull `origin/main` into this fork’s `main`, keep or
adapt fork-only changes after conflict analysis, rebuild the local binary,
confirm `grok-local version` matches the tree, then **review this skill** for
vital updates and show any suggestions to the user before closing.

## Preconditions (abort early if unmet)

Run from the **grok-build** repo root (the tree that contains
`crates/codegen/xai-grok-pager-bin`).

1. Confirm remotes (names may vary — detect, do not hard-fail on labels alone):
   - **Upstream** (xAI monorepo open source): typically `origin` → `xai-org/grok-build`
   - **Fork** (user’s remote): typically `fork` → `status-401-unauthorized/grok-build`
2. Working tree should be clean enough to merge. If dirty:
   - Prefer stashing only if the user did not intentionally leave WIP.
   - If WIP looks intentional, **stop and ask** before discarding or stashing.
3. Preferred branch: local `main` tracking `fork/main`. If on another branch,
   tell the user and ask whether to switch to `main` or update the current
   branch instead.

Record before any mutation:

```bash
git remote -v
git status -sb
git rev-parse --abbrev-ref HEAD
git rev-parse --short HEAD
git rev-parse --short origin/main 2>/dev/null || true
git rev-parse --short fork/main 2>/dev/null || true
```

## Remote / branch model

| Role | Typical remote | Branch |
|------|----------------|--------|
| Upstream source of truth | `origin` | `main` |
| Local integration branch | (local) | `main` |
| Publish target for this fork | `fork` | `main` |

Merge **upstream into local main**, then (only if user asks or skill is run
with push intent) update `fork/main`. Default of this skill: **local merge +
build**; do **not** `git push` without explicit user approval.

## Steps

### 1. Fetch upstream

Record the pre-fetch upstream tip so later steps can isolate **what this sync
actually brought in** (not “fork tree vs origin tree”):

```bash
OLD_ORIGIN=$(git rev-parse origin/main 2>/dev/null || true)
echo "OLD_ORIGIN=${OLD_ORIGIN:-unset}"

git fetch origin main
# Optional but useful for comparison:
git fetch fork main
```

Show how far behind:

```bash
git log --oneline --left-right --cherry-pick HEAD...origin/main | head -40
git rev-list --left-right --count HEAD...origin/main
```

If already up to date with `origin/main` (0 commits to merge), skip merge/conflict
steps and jump to **Step 6 (build)** unless the user only wanted a version rebuild.

### 2. Merge origin/main into local main

Ensure on the integration branch (default `main`):

```bash
git checkout main
PRE_MERGE_HEAD=$(git rev-parse HEAD)
echo "PRE_MERGE_HEAD=$PRE_MERGE_HEAD"
git merge origin/main
```

Commit message style used in this repo when wrapping merges:

```text
Merge origin/main: sync monorepo into fork; <brief note of preserved fork deltas>
```

If the merge completes cleanly, note “no conflicts” and continue to Step 4
with a light fork-delta review (Step 4 still runs — use the pre-merge fork tip
vs merge-base to list unique fork commits).

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
# Commits on HEAD that are not on origin/main (before merge: use pre-merge tip)
git log --oneline origin/main..HEAD   # or: merge-base..fork-tip if mid-merge
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
   headers. Primary paths: `scrollback/blocks/tool/*`, `scrollback/block.rs`,
   `acp/tracker.rs`.
2. **Plugin hooks at spawn** — merge enabled+trusted plugin hooks into the
   session `HookRegistry` at session start (not only on mid-session
   ReloadHooks / ReloadPlugins). Primary path:
   `crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs`
   (look for `append_specs` / “merged plugin hooks into session registry at
   spawn”). Upstream still wires plugin hooks mainly on reload; preserve the
   spawn-time merge unless origin lands an equivalent.

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

How to check (use the **upstream-only** commit range — not
`PRE_MERGE_HEAD..origin/main`):

`PRE_MERGE_HEAD..origin/main` is a tree comparison. When the fork already
diverges in `tool/*`, `tracker.rs`, or `spawn.rs`, those paths show up as
“changed” even if upstream never touched them this sync. Always diff the
commits that landed on origin:

```bash
# Prefer OLD_ORIGIN recorded in Step 1 (pre-fetch origin/main tip).
# Fallback: merge-base of pre-merge HEAD and post-fetch origin/main.
UPSTREAM_BASE="${OLD_ORIGIN:-$(git merge-base PRE_MERGE_HEAD origin/main)}"

# Did this upstream sync touch selection/copy-adjacent code?
git diff --name-only "${UPSTREAM_BASE}"..origin/main -- \
  crates/codegen/xai-grok-pager-render/src/clipboard/ \
  crates/codegen/xai-grok-pager/src/scrollback/ \
  crates/codegen/xai-grok-pager/src/acp/

# Did this upstream sync touch session spawn / plugin-hook wiring?
git diff --name-only "${UPSTREAM_BASE}"..origin/main -- \
  crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs \
  crates/codegen/xai-grok-shell/src/session/acp_session_impl/
```

If any pager watch paths appear, skim the upstream diff for selection ranges,
`CopyDelivery` / toast wiring, and tool failure body handling; confirm
collapsed error suffixes and copyable error text still work (read call sites
or run focused tool-block / selection tests when practical).

If `spawn.rs` (or related hook/plugin helpers) appear, skim the upstream
diff and re-read the post-merge spawn hook-registry block; confirm plugin
hooks are still appended at spawn (not only on reload).

Note both outcomes in the fork-analysis section of the completion report
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

`grok-local` is expected to point at the debug pager binary in this tree:

```bash
# Discover alias target if present
alias grok-local 2>/dev/null || true
type grok-local 2>/dev/null || true
```

Default artifact (adjust if the alias differs):

```text
$REPO/target/debug/xai-grok-pager
```

Build:

```bash
cargo build -p xai-grok-pager-bin
```

Use a long timeout (this crate is large; 15–30+ minutes can be normal on cold
builds). Prefer the workspace toolchain (`rust-toolchain.toml`).

If the build fails:

1. Fix compile errors caused by the merge/adaptation.
2. Rebuild until success.
3. Do not claim success without a successful link of `xai-grok-pager`.

Optional: if `grok-local` is missing from the current shell, define/use the
explicit path for verification:

```bash
HERDR_AGENT=grok "$REPO/target/debug/xai-grok-pager" version
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
VERSION_WITH_COMMIT = "{CARGO_PKG_VERSION|GROK_VERSION} ({git rev-parse --short HEAD})"
```

Channel suffix (`[stable]`, `[alpha]`, …) comes from runtime update config via
`xai_grok_update::channel_label()` — do not treat channel mismatch as a version
failure if semver + commit match.

Verification:

```bash
EXPECTED=$(grep -E '^version = ' crates/codegen/xai-grok-version/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
SHORT=$(git rev-parse --short HEAD)
echo "Expected semver: $EXPECTED  short HEAD: $SHORT"

# Prefer alias when available
if alias grok-local >/dev/null 2>&1 || command -v grok-local >/dev/null 2>&1; then
  OUT=$(grok-local version 2>&1)
else
  OUT=$(HERDR_AGENT=grok ./target/debug/xai-grok-pager version 2>&1)
fi
echo "$OUT"

# Pass criteria: output contains EXPECTED and SHORT (commit is baked at build time)
echo "$OUT" | grep -F "$EXPECTED" && echo "$OUT" | grep -F "$SHORT"
```

**Pass:** `grok-local version` (or equivalent path) shows `grok $EXPECTED ($SHORT) …`

**Fail if:**

- Still shows an older semver (stale binary / wrong path).
- Commit hash is an old build’s hash (binary not rebuilt after merge).
- Binary path is not the one just built (check alias → absolute path).

On fail: confirm alias path, `ls -l` binary mtime, rebuild with
`cargo clean -p xai-grok-pager-bin` only if incremental link is wrong, then
re-run verification.

### 8. Review this skill (mandatory before closing)

After the build/version work is done (success or documented failure), re-read
this file:

```text
.grok/skills/update-grok-local/SKILL.md
```

Compare the skill’s assumptions to **what just happened** in this run and to
any upstream changes that affect the procedure. Goal: catch vital drift so the
next `/update-grok-local` stays accurate.

**Check at least:**

| Area | Stale if… |
|------|-----------|
| Remotes / branches | Remote names or tracking model changed |
| Package / binary paths | `xai-grok-pager-bin`, artifact path, or `grok-local` wiring changed |
| Version sources | Semver crate, `build.rs` embed, or channel labeling changed |
| Fork themes | Upstream absorbed error-UI or plugin-hooks-at-spawn, or a new deliberate fork theme appeared |
| Adjacent watch paths | New surfaces matter for copy/selection/tool-error or plugin-hook spawn intent (clipboard, scrollback, ACP, `spawn.rs`, …) |
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

1. **Sync:** pre/post SHAs (`HEAD`, `origin/main`), commits merged count.
2. **Conflicts:** files (or “none”), resolution summary.
3. **Fork analysis:** each delta → keep / adapt / drop + one-line rationale;
   include **adjacent re-check** results when upstream touched clipboard /
   selection / tool-block paths and/or session spawn / plugin-hook wiring
   (or “n/a — paths untouched” per theme).
4. **Code adjustments:** what was implemented after analysis (or “none”).
5. **Build:** success/fail, package `xai-grok-pager-bin`, binary path + mtime.
6. **Version check:** full `grok-local version` line vs expected `$EXPECTED ($SHORT)`.
7. **Next steps (optional):** push to `fork/main` only if user wants:
   `git push fork main` (require confirmation — shared remote).
8. **Skill review:** “no vital updates” or numbered suggestions (Step 8). Never
   omit this section.

## Safety rules

- Never force-push to `origin` or `fork` unless the user explicitly requests it.
- Never `git reset --hard` or discard uncommitted work without confirmation.
- Prefer resolving conflicts over aborting; abort only on user request or
  unrecoverable state.
- Do not skip Step 4 (analysis) when there were conflicts or unique fork
  commits.
- Do not claim version success without running the version command against the
  binary that was just built.
- Do not skip Step 8 (skill review). Do not silently edit this skill; propose
  and wait for the user.

## Quick reference

```bash
# Full happy path (agent expands conflict/analysis + skill review as needed)
OLD_ORIGIN=$(git rev-parse origin/main)
git fetch origin main
git checkout main
PRE_MERGE_HEAD=$(git rev-parse HEAD)
git merge origin/main   # resolve + analyze if needed
# Adjacent re-check (pager + spawn/plugin-hooks): git diff --name-only $OLD_ORIGIN..origin/main -- <watch paths>
cargo build -p xai-grok-pager-bin
grok-local version      # or HERDR_AGENT=grok ./target/debug/xai-grok-pager version
# then: re-read this SKILL.md → report skill-review suggestions (or none)
```
