#!/bin/sh
# >>> git-paw managed hook >>>
# Branch guard: refuse a commit that would advance a branch other than
# the one this worktree was created for (cross-worktree contamination).
# git does not reliably export GIT_DIR to pre-commit, so resolve the
# per-worktree gitdir via rev-parse with a GIT_DIR fallback.
PAW_GD="${GIT_DIR:-$(git rev-parse --git-dir 2>/dev/null)}"
if [ -n "$PAW_GD" ] && [ -f "$PAW_GD/paw-agent-id" ]; then
. "$PAW_GD/paw-agent-id"
if [ -n "$PAW_EXPECTED_BRANCH" ] && [ "$PAW_STRICT_BRANCH_GUARD" != "false" ]; then
PAW_CUR=$(git symbolic-ref --short HEAD 2>/dev/null)
if [ -n "$PAW_CUR" ] && [ "$PAW_CUR" != "$PAW_EXPECTED_BRANCH" ]; then
echo "error: git-paw branch guard refused this commit" >&2
echo "  HEAD is on '$PAW_CUR' but this worktree is for '$PAW_EXPECTED_BRANCH'." >&2
echo "  The commit would advance the wrong branch. Switch back to '$PAW_EXPECTED_BRANCH'" >&2
echo "  (or set [supervisor] strict_branch_guard = false to override)." >&2
exit 1
fi
fi
fi
# <<< git-paw managed hook <<<
