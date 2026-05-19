#!/usr/bin/env bash
# Creates the planned GitHub issues for this repository.
# The script exists because GitHub CLI authentication/network access can fail in
# local agent environments; once `gh auth status` is healthy, it imports the
# issue backlog from docs/github-issues-backlog.md with consistent labels.

set -euo pipefail

if ! gh auth status >/dev/null 2>&1; then
  echo "gh is not authenticated. Run: gh auth login -h github.com" >&2
  exit 1
fi

backlog_file="${1:-docs/github-issues-backlog.md}"

if [[ ! -f "$backlog_file" ]]; then
  echo "backlog file not found: $backlog_file" >&2
  exit 1
fi

while IFS= read -r line; do
  [[ "$line" =~ ^[0-9]+\. ]] || continue
  title="${line#*. }"
  label_segment="${title%%]*}]"
  labels=()

  while [[ "$label_segment" =~ ^\[([^]]+)\](.*)$ ]]; do
    labels+=("${BASH_REMATCH[1]}")
    label_segment="${BASH_REMATCH[2]}"
  done

  body="Imported from docs/github-issues-backlog.md.

Scope:
- ${title}

Acceptance criteria:
- Implementation is covered by tests or an explicit manual verification note.
- Money-state behavior is idempotent where applicable.
- Logs and audit events avoid leaking PII or secrets.
- Documentation or runbook updates are included when operator behavior changes."

  args=(issue create --title "$title" --body "$body")
  for label in "${labels[@]}"; do
    args+=(--label "$label")
  done

  echo "Creating issue: $title"
  gh "${args[@]}"
done < "$backlog_file"
