#!/bin/sh
# >>> git-paw managed hook >>>
if [ -n "$GIT_DIR" ] && [ -f "$GIT_DIR/paw-agent-id" ]; then
echo 'error: git-paw agents must not push. The supervisor handles merges.' >&2
exit 1
fi
# <<< git-paw managed hook <<<
