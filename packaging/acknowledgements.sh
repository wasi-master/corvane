#!/usr/bin/env bash
# Regenerate assets/acknowledgements.json (About › License and Open Source
# Notices, GHD `static/licenses.json`) from `cargo about`. Run after changing
# dependencies and commit the result; the app never runs cargo-about.
#
#   cargo install cargo-about --locked --features cli   # once
#   packaging/acknowledgements.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/assets/acknowledgements.json"
RAW="$(mktemp)"
trap 'rm -f "$RAW"' EXIT

command -v cargo-about >/dev/null || {
  echo "cargo-about not found: cargo install cargo-about --locked --features cli" >&2
  exit 1
}

cd "$ROOT"
cargo about generate --format json --config packaging/about.toml \
  --manifest-path "$ROOT/crates/corvane/Cargo.toml" -o "$RAW"

# Compact form: each license text once, each crate once (sorted by name), and
# Corvane's own LICENSE.
python3 - "$RAW" "$ROOT/LICENSE" "$OUT" <<'PY'
import json, sys

raw_path, license_path, out_path = sys.argv[1:4]
raw = json.load(open(raw_path))
texts, text_ix, crates = [], {}, {}
for lic in raw["licenses"]:
    text = lic["text"].strip()
    if text not in text_ix:
        text_ix[text] = len(texts)
        texts.append(text)
    for used in lic["used_by"]:
        krate = used["crate"]
        if krate["name"].startswith("corvane"):
            continue
        key = (krate["name"], krate["version"])
        entry = crates.setdefault(key, {
            "name": krate["name"],
            "version": krate["version"],
            "repository": krate.get("repository") or krate.get("homepage"),
            "license": krate.get("license") or lic["id"],
            "texts": [],
        })
        if text_ix[text] not in entry["texts"]:
            entry["texts"].append(text_ix[text])
out = {
    "app_license": open(license_path).read().strip(),
    "texts": texts,
    "crates": sorted(crates.values(), key=lambda c: (c["name"].lower(), c["version"])),
}
with open(out_path, "w") as f:
    json.dump(out, f, separators=(",", ":"), ensure_ascii=False)
    f.write("\n")
print(f"{out_path}: {len(out['crates'])} crates, {len(texts)} license texts")
PY
