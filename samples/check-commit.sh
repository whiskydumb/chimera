#!/usr/bin/env bash
# pre-commit guard: format, lint, and validate the commit message.
set -euo pipefail

readonly MSG_FILE="${1:-.git/COMMIT_EDITMSG}"
readonly PATTERN='^(feat|fix|refactor|docs|test|chore)(\(.+\))?: .{1,72}'

if command -v cargo >/dev/null 2>&1; then
  cargo fmt --all -- --check
  cargo clippy --all-targets -- -D warnings
fi

if ! grep -Eq "${PATTERN}" "${MSG_FILE}"; then
  printf 'commit message must follow conventional commits\n' >&2
  exit 1
fi
