#!/bin/bash

set -euo pipefail

src_dir="$(pwd -P)"
dst_dir="$HOME/.config/opencode/commands"

mkdir -p "$dst_dir"

shopt -s nullglob
for f in "$src_dir"/*.md; do
  base="$(basename "$f")"
  ln -sf "$f" "$dst_dir/$base"
done
