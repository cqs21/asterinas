#!/usr/bin/env bash
# submit_review.sh - Parse reviews.md and submit to GitHub as a pending review.
#
# Usage: bash submit_review.sh <path_to_reviews.md>
#
# This script:
# 1. Parses the YAML frontmatter for pr, repo, head_sha, event
# 2. Extracts the Summary section as the review body
# 3. Parses each ## comment section into GitHub API comment objects
# 4. Checks for staleness (head_sha vs current PR HEAD)
# 5. POSTs a pending review via the GitHub API

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <path_to_reviews.md>"
    exit 1
fi

REVIEWS_FILE="$1"

if [ ! -f "$REVIEWS_FILE" ]; then
    echo "Error: $REVIEWS_FILE not found."
    echo "Run '/pr-review new <N>' first to generate the review file."
    exit 1
fi

# --- Parse frontmatter ---
# Extract YAML between first pair of --- lines
FRONTMATTER=$(awk '/^---$/{n++; next} n==1{print} n==2{exit}' "$REVIEWS_FILE")

get_field() {
    echo "$FRONTMATTER" | grep "^${1}:" | sed "s/^${1}:[[:space:]]*//" | sed 's/^"\(.*\)"$/\1/' | sed "s/^'\(.*\)'$/\1/"
}

PR_NUMBER=$(get_field "pr")
REPO=$(get_field "repo")
HEAD_SHA=$(get_field "head_sha")
EVENT=$(get_field "event")

# Validate required fields
MISSING=""
[ -z "$PR_NUMBER" ] && MISSING="$MISSING pr"
[ -z "$REPO" ] && MISSING="$MISSING repo"
[ -z "$HEAD_SHA" ] && MISSING="$MISSING head_sha"
if [ -n "$MISSING" ]; then
    echo "Error: Missing required frontmatter fields:$MISSING"
    exit 1
fi

# Default event to comment
EVENT="${EVENT:-comment}"

# Map event to GitHub API event string (informational only; review is
# always created as PENDING so the user can finalize on GitHub)
case "$EVENT" in
    comment)         API_EVENT="COMMENT" ;;
    approve)         API_EVENT="APPROVE" ;;
    request_changes) API_EVENT="REQUEST_CHANGES" ;;
    *)
        echo "Error: Invalid event '$EVENT'. Must be: comment, approve, request_changes"
        exit 1
        ;;
esac

echo "PR:       #$PR_NUMBER"
echo "Repo:     $REPO"
echo "HEAD SHA: $HEAD_SHA"
echo "Event:    $EVENT -> $API_EVENT"
echo ""

# --- Check staleness ---
CURRENT_SHA=$(gh pr view "$PR_NUMBER" --repo "$REPO" --json headRefOid -q .headRefOid 2>/dev/null || echo "")

if [ -z "$CURRENT_SHA" ]; then
    echo "Warning: Could not fetch current PR HEAD. Proceeding anyway."
elif [ "$CURRENT_SHA" != "$HEAD_SHA" ]; then
    echo "Error: PR has been updated since this review was generated."
    echo "  Review's head_sha: $HEAD_SHA"
    echo "  Current head_sha:  $CURRENT_SHA"
    echo ""
    echo "Run '/pr-review redo $PR_NUMBER' to generate a follow-up review."
    exit 1
else
    echo "Staleness check: OK (head_sha matches)"
fi

# --- Extract summary ---
# Everything between "# Summary" and the next "---"
SUMMARY=$(awk '
    /^# Summary[[:space:]]*$/ { capture=1; next }
    capture && /^---[[:space:]]*$/ { exit }
    capture { print }
' "$REVIEWS_FILE" | sed -e 's/^[[:space:]]*//' -e '/./,$!d' | sed -e :a -e '/^[[:space:]]*$/{ $d; N; ba; }')

if [ -z "$SUMMARY" ]; then
    echo "Warning: No summary section found. Using empty review body."
    SUMMARY=""
fi

# --- Parse comment sections ---
# We use Python for reliable markdown parsing since the format is non-trivial.
COMMENTS_JSON=$(REVIEWS_FILE="$REVIEWS_FILE" python3 << 'PYEOF'
import re
import json
import os

reviews_file = os.environ["REVIEWS_FILE"]

with open(reviews_file, 'r') as f:
    content = f.read()

# Split into sections by ---
# First, skip the frontmatter (between first two ---)
parts = content.split('\n')
frontmatter_count = 0
body_start = 0
for i, line in enumerate(parts):
    if line.strip() == '---':
        frontmatter_count += 1
        if frontmatter_count == 2:
            body_start = i + 1
            break

body = '\n'.join(parts[body_start:])

# Split body into sections by --- (horizontal rules)
sections = re.split(r'\n---\s*\n', body)

# Heading pattern: ## `path/to/file` line 42
# or: ## `path/to/file` lines 40-42
# or: ## `path/to/file` line 42 (old)
# or: ## `path/to/file`
heading_re = re.compile(
    r'^##\s+`([^`]+)`'           # path in backticks
    r'(?:\s+lines?\s+'           # optional line/lines keyword
    r'(\d+)(?:-(\d+))?'          # line number, optional range end
    r')?'
    r'(\s+\(old\))?'             # optional (old) suffix
    r'\s*$',
    re.MULTILINE
)

comments = []
skipped = 0

for section in sections:
    section = section.strip()
    if not section:
        continue

    match = heading_re.search(section)
    if not match:
        continue

    path = match.group(1)
    line_start = match.group(2)
    line_end = match.group(3)
    is_old = match.group(4) is not None

    # Extract body: everything after the heading, skipping blockquoted lines
    after_heading = section[match.end():].strip()
    lines = after_heading.split('\n')

    # Skip blockquoted diff blocks (lines starting with >)
    body_lines = []
    in_blockquote = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith('> '):
            in_blockquote = True
            continue
        if in_blockquote and stripped == '>':
            continue
        if in_blockquote and not stripped.startswith('>'):
            in_blockquote = False
        if not stripped.startswith('>'):
            body_lines.append(line)

    body_text = '\n'.join(body_lines).strip()

    if not body_text:
        skipped += 1
        continue

    comment = {"path": path, "body": body_text}

    if line_start:
        comment["line"] = int(line_end) if line_end else int(line_start)
        comment["side"] = "LEFT" if is_old else "RIGHT"

        if line_end and line_start != line_end:
            comment["start_line"] = int(line_start)
            comment["start_side"] = "LEFT" if is_old else "RIGHT"
    else:
        comment["subject_type"] = "file"

    comments.append(comment)

output = {"comments": comments, "skipped": skipped}
print(json.dumps(output))
PYEOF
)

NUM_COMMENTS=$(echo "$COMMENTS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d['comments']))")
NUM_SKIPPED=$(echo "$COMMENTS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['skipped'])")
COMMENTS_ARRAY=$(echo "$COMMENTS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(json.dumps(d['comments']))")

echo ""
echo "Parsed $NUM_COMMENTS comment(s), skipped $NUM_SKIPPED empty comment(s)."

if [ "$NUM_COMMENTS" -eq 0 ] && [ -z "$SUMMARY" ]; then
    echo "Nothing to submit (no summary, no comments)."
    exit 0
fi

# --- Submit review ---
echo ""
echo "Submitting review to GitHub..."

echo "  Total comments: $NUM_COMMENTS"
[ "$NUM_SKIPPED" -gt 0 ] && echo "  Empty (skipped): $NUM_SKIPPED"

# --- Create or find the pending review ---
# Comments are always added via GraphQL addPullRequestReviewThread after the
# review exists. The REST create-review endpoint's inline comment support
# does not reliably position comments on diff lines (gh translates REST to
# GraphQL internally, and DraftPullRequestReviewThread lacks line/side fields).
EXISTING_REVIEW_ID=$(gh api "repos/$REPO/pulls/$PR_NUMBER/reviews" \
    --jq '.[] | select(.state == "PENDING") | .id' 2>/dev/null | head -n1 || echo "")

PRE_EXISTING_COMMENT_IDS="[]"

if [ -n "$EXISTING_REVIEW_ID" ]; then
    echo ""
    echo "Found existing pending review (ID: $EXISTING_REVIEW_ID). Appending to it."

    REVIEW_ID="$EXISTING_REVIEW_ID"

    # Get the GraphQL node_id for the existing review (required by GraphQL mutations)
    REVIEW_NODE_ID=$(gh api "repos/$REPO/pulls/$PR_NUMBER/reviews/$EXISTING_REVIEW_ID" \
        --jq '.node_id' 2>/dev/null || echo "")

    if [ -z "$REVIEW_NODE_ID" ]; then
        echo "Error: Could not fetch node_id for existing review."
        exit 1
    fi

    # Capture comment IDs that already exist on this review, so the post-submit
    # verification step can distinguish "landed in this batch" from "was already
    # there". Without this, a pre-existing comment on (path, line) would be
    # credited as a successful match for a new comment on the same coordinates.
    PRE_EXISTING_COMMENT_IDS=$(gh api graphql -f query="
        query {
          node(id: \"$REVIEW_NODE_ID\") {
            ... on PullRequestReview {
              comments(first: 100) { nodes { id } }
            }
          }
        }
    " --jq '[.data.node.comments.nodes[].id]' 2>/dev/null || echo "[]")

    # Update the review body (summary) via GraphQL if we have a non-empty summary
    if [ -n "$SUMMARY" ]; then
        ESCAPED_SUMMARY=$(python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "$SUMMARY")
        UPDATE_RESP=$(gh api graphql -f query="mutation {
            updatePullRequestReview(input: {
                pullRequestReviewId: \"$REVIEW_NODE_ID\",
                body: $ESCAPED_SUMMARY
            }) { pullRequestReview { id } }
        }" 2>&1) || {
            echo "  Note: Could not update review body (GitHub does not allow updating"
            echo "  a review that was created without a body). You can add the summary"
            echo "  manually when finalizing the review on GitHub."
        }
    fi
else
    # No existing pending review -- create one with summary only (no comments).
    PAYLOAD=$(PR_SUMMARY="$SUMMARY" PR_HEAD_SHA="$HEAD_SHA" python3 -c "
import json, os
print(json.dumps({
    'commit_id': os.environ['PR_HEAD_SHA'],
    'body': os.environ['PR_SUMMARY'],
    'comments': []
}))
")

    RESPONSE=$(echo "$PAYLOAD" | gh api \
        "repos/$REPO/pulls/$PR_NUMBER/reviews" \
        --method POST \
        --input - 2>&1) || {
        echo ""
        echo "Error: GitHub API call failed."
        echo "$RESPONSE"
        exit 1
    }

    REVIEW_ID=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id', 'unknown'))" 2>/dev/null || echo "unknown")
    REVIEW_NODE_ID=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('node_id', ''))" 2>/dev/null || echo "")

    if [ -z "$REVIEW_NODE_ID" ]; then
        # Fetch node_id separately if not in the create response
        REVIEW_NODE_ID=$(gh api "repos/$REPO/pulls/$PR_NUMBER/reviews/$REVIEW_ID" \
            --jq '.node_id' 2>/dev/null || echo "")
    fi

    if [ -z "$REVIEW_NODE_ID" ]; then
        echo "Error: Could not determine node_id for new review."
        exit 1
    fi
fi

# --- Add all comments via GraphQL ---
# Each comment is added as a review thread using addPullRequestReviewThread,
# which reliably supports line/side positioning for inline comments and
# subjectType: FILE for file-level comments.
if [ "$NUM_COMMENTS" -gt 0 ]; then
    COMMENTS_TMPFILE=$(mktemp)
    echo "$COMMENTS_ARRAY" | python3 -c "
import sys, json
comments = json.load(sys.stdin)
for c in comments:
    print(json.dumps(c))
" > "$COMMENTS_TMPFILE"

    ADDED=0
    FAILED=0
    while IFS= read -r COMMENT_JSON; do
        # Build the GraphQL mutation for this comment
        GQL_RESULT=$(echo "$COMMENT_JSON" | REVIEW_NODE_ID="$REVIEW_NODE_ID" python3 -c "
import sys, json, os

c = json.load(sys.stdin)
review_id = os.environ['REVIEW_NODE_ID']
path = c['path']
body = json.dumps(c['body'])

# File-level comments: use subject_type FILE (no line)
if c.get('subject_type') == 'file':
    mutation = '''mutation {
        addPullRequestReviewThread(input: {
            pullRequestReviewId: \"%s\",
            body: %s,
            path: \"%s\",
            subjectType: FILE
        }) { thread { id } }
    }''' % (review_id, body, path)
else:
    line = c.get('line', 1)
    side = c.get('side', 'RIGHT')
    start_line = c.get('start_line')
    start_side = c.get('start_side')

    if start_line and start_line != line:
        mutation = '''mutation {
            addPullRequestReviewThread(input: {
                pullRequestReviewId: \"%s\",
                body: %s,
                path: \"%s\",
                line: %d,
                side: %s,
                startLine: %d,
                startSide: %s
            }) { thread { id } }
        }''' % (review_id, body, path, line, side, start_line, start_side)
    else:
        mutation = '''mutation {
            addPullRequestReviewThread(input: {
                pullRequestReviewId: \"%s\",
                body: %s,
                path: \"%s\",
                line: %d,
                side: %s
            }) { thread { id } }
        }''' % (review_id, body, path, line, side)

print(mutation)
" 2>&1) || {
            FAILED=$((FAILED + 1))
            COMMENT_PATH=$(echo "$COMMENT_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin).get('path','?'))")
            echo "  Warning: Failed to build mutation for $COMMENT_PATH"
            continue
        }

        RESP=$(gh api graphql -f query="$GQL_RESULT" 2>&1) && {
            ADDED=$((ADDED + 1))
        } || {
            FAILED=$((FAILED + 1))
            COMMENT_PATH=$(echo "$COMMENT_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin).get('path','?'))")
            echo "  Warning: Failed to add comment on $COMMENT_PATH: $RESP"
        }
    done < "$COMMENTS_TMPFILE"
    rm -f "$COMMENTS_TMPFILE"

    echo ""
    echo "  Added $ADDED comment(s)."
    [ "$FAILED" -gt 0 ] && echo "  Failed to add $FAILED comment(s)."
fi

# --- Verify submitted comments ---
# Re-query the review via GraphQL and match (path, line, body-prefix) against
# the expected list. Catches silent failures like:
#   - Line number outside the PR diff hunks (GraphQL accepts but drops thread)
#   - File-level comment on a path not in the PR's tree
#   - Body truncation or URL-encoding issues
#
# When appending to an existing pending review, pre-existing comments are
# excluded from the "actual" set so they can't absorb matches for the new
# batch (see PRE_EXISTING_COMMENT_IDS capture above).
if [ "$NUM_COMMENTS" -gt 0 ]; then
    echo ""
    echo "Verifying submitted comments..."

    VERIFY_QUERY_ERR=$(mktemp)
    ACTUAL_THREADS=$(gh api graphql -f query="
        query {
          node(id: \"$REVIEW_NODE_ID\") {
            ... on PullRequestReview {
              comments(first: 100) {
                nodes {
                  id
                  path
                  line
                  originalLine
                  startLine
                  originalStartLine
                  subjectType
                  body
                }
              }
            }
          }
        }
    " --jq '.data.node.comments.nodes' 2>"$VERIFY_QUERY_ERR")
    VERIFY_QUERY_STATUS=$?

    if [ "$VERIFY_QUERY_STATUS" -ne 0 ] || [ -z "$ACTUAL_THREADS" ]; then
        echo "  Warning: could not verify (GraphQL query failed). Comments may or"
        echo "  may not be present on the review. Check the review on GitHub."
        if [ -s "$VERIFY_QUERY_ERR" ]; then
            sed 's/^/    /' "$VERIFY_QUERY_ERR"
        fi
        rm -f "$VERIFY_QUERY_ERR"
    else
        rm -f "$VERIFY_QUERY_ERR"

        VERIFY_JSON=$(ACTUAL="$ACTUAL_THREADS" EXPECTED="$COMMENTS_ARRAY" PRE_IDS="$PRE_EXISTING_COMMENT_IDS" python3 <<'PYEOF'
import json, os

actual = json.loads(os.environ.get("ACTUAL") or "[]")
expected = json.loads(os.environ.get("EXPECTED") or "[]")
pre_ids = set(json.loads(os.environ.get("PRE_IDS") or "[]"))

# Exclude pre-existing comments so they can't absorb matches for the new batch.
actual = [a for a in actual if a.get("id") not in pre_ids]

def actual_line(c):
    # GitHub may populate line or originalLine depending on rebase state.
    return c.get("line") or c.get("originalLine")

def body_prefix(text):
    return " ".join((text or "").split())[:80]

missing = []
used = [False] * len(actual)
for e in expected:
    ep = e["path"]
    el = e.get("line")
    file_level = e.get("subject_type") == "file"
    exp_prefix = body_prefix(e["body"])
    found_idx = None
    for i, a in enumerate(actual):
        if used[i] or a.get("path") != ep:
            continue
        if file_level:
            if a.get("subjectType") == "FILE" or actual_line(a) is None:
                if body_prefix(a.get("body", "")).startswith(exp_prefix[:40]):
                    found_idx = i
                    break
        else:
            if actual_line(a) == el and body_prefix(a.get("body", "")).startswith(exp_prefix[:40]):
                found_idx = i
                break
    if found_idx is None:
        missing.append({"path": ep, "line": el, "file_level": file_level})
    else:
        used[found_idx] = True

print(json.dumps({
    "expected": len(expected),
    "actual_in_batch": len(actual),
    "missing": missing,
}))
PYEOF
)

        V_EXPECTED=$(echo "$VERIFY_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin)['expected'])")
        V_ACTUAL=$(echo "$VERIFY_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin)['actual_in_batch'])")
        V_MISSING_N=$(echo "$VERIFY_JSON" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['missing']))")

        echo "  Expected in this batch: $V_EXPECTED; landed on GitHub: $V_ACTUAL"
        if [ "$V_MISSING_N" -gt 0 ]; then
            echo "  WARNING: $V_MISSING_N expected comment(s) not found on the review:"
            echo "$VERIFY_JSON" | python3 -c "
import sys, json
for m in json.load(sys.stdin)['missing']:
    where = 'file-level' if m['file_level'] else ('line ' + str(m['line']))
    print(f\"    - {m['path']} ({where})\")
"
            echo ""
            echo "  Common causes:"
            echo "    - Line number is not part of the PR diff (GitHub only accepts inline"
            echo "      comments on lines inside a diff hunk). Re-anchor to a line in the diff."
            echo "    - File path has a case mismatch or is not in the PR's tree."
            echo "    - GraphQL accepted the mutation but silently dropped the thread."
            echo ""
            echo "  Add the missing comments manually on GitHub, or edit the review file to"
            echo "  fix the offending comments and re-run '/pr-review submit $PR_NUMBER'."
        else
            echo "  All expected comments present."
        fi
    fi
fi

echo ""
echo "Review submitted successfully (review ID: $REVIEW_ID)"
echo "  State: PENDING"
echo ""
echo "Go to GitHub to finalize the review:"
echo "  https://github.com/$REPO/pull/$PR_NUMBER"
echo ""
echo "The review is in PENDING state. On GitHub you can:"
echo "  - Add more comments"
echo "  - Edit existing comments"
echo "  - Submit as: Comment, Approve, or Request Changes"
