#!/usr/bin/env bash

set -e

if [[ $# -ne 0 ]]; then
  echo >&2 "usage: $(basename "$0")"
  exit 1
fi

dver="$(grep LABEL Dockerfile | grep image.version | awk -F= '{print $2}')"
cver="$(grep '^version' Cargo.toml | awk -F\" '{print $2}')"
hver="$(grep '<span class="version">' index.html | awk -F\> '{print $2}' |
  awk -F\< '{print $1}')"

if [[ $dver != "v$cver" ]]; then
  echo >&2 "error: version mismatch"

  echo "dockerfile: $dver"
  echo "cargo: $cver"

  exit 1
fi

if [[ $hver != "v$cver" ]]; then
  echo >&2 "error: version mismatch"

  echo "index.html: $hver"
  echo "cargo: $cver"

  exit 1
fi

exit 0
