#!/usr/bin/env bash
set -euo pipefail

LCOV_FILE="${1:-}"
THRESHOLD="${2:-80}"
LABEL="${3:-coverage}"

if [[ -z "$LCOV_FILE" ]]; then
  echo "Usage: bash scripts/check_lcov_threshold.sh <lcov-file> [threshold] [label]" >&2
  exit 2
fi

if [[ ! -f "$LCOV_FILE" ]]; then
  echo "LCOV file not found: $LCOV_FILE" >&2
  exit 2
fi

TOTAL_LINES="$(awk -F: '/^LF:/ { total += $2 } END { print total + 0 }' "$LCOV_FILE")"
COVERED_LINES="$(awk -F: '/^LH:/ { covered += $2 } END { print covered + 0 }' "$LCOV_FILE")"

COVERAGE="$(awk -v total="$TOTAL_LINES" -v covered="$COVERED_LINES" '
  BEGIN {
    if (total == 0) {
      printf "0.00"
    } else {
      printf "%.2f", (covered / total) * 100
    }
  }
')"

echo "Coverage (${LABEL}): ${COVERAGE}% (${COVERED_LINES}/${TOTAL_LINES})"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "## Coverage Summary"
    echo
    echo "| Scope | Coverage | Covered Lines | Total Lines | Threshold |"
    echo "| --- | ---: | ---: | ---: | ---: |"
    echo "| ${LABEL} | ${COVERAGE}% | ${COVERED_LINES} | ${TOTAL_LINES} | ${THRESHOLD}% |"
  } >> "${GITHUB_STEP_SUMMARY}"
fi

awk -v coverage="$COVERAGE" -v threshold="$THRESHOLD" '
  BEGIN {
    if (coverage + 0 < threshold + 0) {
      printf "ERROR: Coverage %s%% is below %s%% threshold.\n", coverage, threshold
      exit 1
    }

    printf "SUCCESS: Coverage %s%% meets %s%% threshold.\n", coverage, threshold
  }
'
