#!/bin/sh
# >>> git-paw managed hook >>>
# Dispatcher: reads the per-worktree paw-agent-id marker and publishes
# agent.artifact to the git-paw broker. Resolve the gitdir via
# rev-parse with a GIT_DIR fallback (git does not always export it).
PAW_GD="${GIT_DIR:-$(git rev-parse --git-dir 2>/dev/null)}"
if [ -n "$PAW_GD" ] && [ -f "$PAW_GD/paw-agent-id" ]; then
. "$PAW_GD/paw-agent-id"
FILES=$(git diff HEAD~1 --name-only 2>/dev/null | awk '{printf "%s\"%s\"", (NR>1?",":""), $0}')
curl -s -X POST "$PAW_BROKER_URL/publish" \
-H 'Content-Type: application/json' \
-d "{\"type\":\"agent.artifact\",\"agent_id\":\"$PAW_AGENT_ID\",\"payload\":{\"status\":\"committed\",\"exports\":[],\"modified_files\":[$FILES]}}" \
>/dev/null 2>&1 || true
# Branch-mismatch detection (detection without enforcement — fires
# regardless of PAW_STRICT_BRANCH_GUARD; the pre-commit hook owns
# blocking). Publishes agent.feedback + an agent.learning record
# (category permission_pattern) identifying the contamination.
if [ -n "$PAW_EXPECTED_BRANCH" ]; then
PAW_CUR=$(git symbolic-ref --short HEAD 2>/dev/null)
if [ -n "$PAW_CUR" ] && [ "$PAW_CUR" != "$PAW_EXPECTED_BRANCH" ]; then
PAW_SHA=$(git rev-parse HEAD 2>/dev/null)
curl -s -X POST "$PAW_BROKER_URL/publish" \
-H 'Content-Type: application/json' \
-d "{\"type\":\"agent.feedback\",\"agent_id\":\"$PAW_AGENT_ID\",\"payload\":{\"from\":\"branch-guard\",\"errors\":[\"commit $PAW_SHA advanced '$PAW_CUR' but this worktree is for '$PAW_EXPECTED_BRANCH'; cherry-pick onto '$PAW_EXPECTED_BRANCH' and reset '$PAW_CUR'\"]}}" \
>/dev/null 2>&1 || true
curl -s -X POST "$PAW_BROKER_URL/publish" \
-H 'Content-Type: application/json' \
-d "{\"type\":\"agent.learning\",\"agent_id\":\"$PAW_AGENT_ID\",\"payload\":{\"category\":\"permission_pattern\",\"body\":\"cross-worktree contamination: commit $PAW_SHA landed on '$PAW_CUR' instead of expected '$PAW_EXPECTED_BRANCH'\"}}" \
>/dev/null 2>&1 || true
fi
fi
fi
# <<< git-paw managed hook <<<
