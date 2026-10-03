#!/usr/bin/env bats
# Tests for the release helper script.

load 'test_helper/bats-support/load'
load 'test_helper/bats-assert/load'

SCRIPT="${BATS_TEST_DIRNAME}/../bin/release"

setup_file() { export RELEASE_DRY_RUN=1; }

setup() {
  TMP="$(mktemp -d)"
  cd "$TMP" || return 1
  git init --quiet && git commit --allow-empty --quiet -m 'init'
}

teardown() {
  rm -rf "$TMP"
}

@test "prints usage without arguments" {
  run "$SCRIPT"
  [ "$status" -eq 2 ]
  [[ "$output" == *"usage: release"* ]]
}

@test "rejects an invalid version: 1.x" {
  run "$SCRIPT" 1.x
  assert_failure
  assert_output --partial "invalid version '1.x'"
}

@test "tags the current commit" {
  run "$SCRIPT" 1.2.3
  assert_success
  local tags
  tags=$(git tag --list 'v*' | wc -l)
  (( tags == 1 )) || fail "expected one tag, got $tags"
}

@test "skips on CI" {
  [[ -n "${CI:-}" ]] && skip "not run on CI"
  run -127 some-missing-command
  echo -e "status=${status}\tlines=${#lines[@]}" >&3
}
