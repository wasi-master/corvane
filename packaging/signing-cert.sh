#!/usr/bin/env bash
# The self-signed code-signing certificate Corvane.app is signed with
# (packaging/release.md, "Code-signing certificate").
#
#   packaging/signing-cert.sh create [dir]   # new certificate → login keychain, .p12 + password in dir
#   packaging/signing-cert.sh import <p12>   # an existing .p12 → login keychain (another machine)
#   packaging/signing-cert.sh ci             # CI: $MACOS_SIGNING_P12 (base64) → a temporary keychain
#
# Not an Apple Developer ID: Gatekeeper still rejects a quarantined download.
# What it buys is a stable designated requirement (identifier + this
# certificate) instead of the ad-hoc one (the binary's hash), so Keychain
# items and privacy permissions granted to one build carry over to the next.
# KEYCHAIN=<path> imports into another keychain (testing).
set -euo pipefail

NAME="Corvane Self-Signed"
OPENSSL=/usr/bin/openssl # LibreSSL: its .p12 files import without -legacy
KEYCHAIN="${KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}"

has_identity() {
  security find-identity -p codesigning "$@" 2>/dev/null | grep -q "\"$NAME\""
}

import_p12() {
  local p12="$1" password="$2" keychain="$3"
  security import "$p12" -k "$keychain" -f pkcs12 -P "$password" -T /usr/bin/codesign >/dev/null
}

case "${1:-}" in
  create)
    DIR="${2:-$HOME/.corvane-signing}"
    if has_identity "$KEYCHAIN"; then
      echo "\"$NAME\" is already in $KEYCHAIN; delete it in Keychain Access to make a new one" >&2
      exit 1
    fi
    [[ -e "$DIR/corvane-signing.p12" ]] && { echo "$DIR/corvane-signing.p12 exists; import it instead" >&2; exit 1; }
    mkdir -p "$DIR" && chmod 700 "$DIR"
    WORK="$(mktemp -d)"
    trap 'rm -rf "$WORK"' EXIT
    cat > "$WORK/cert.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
EOF
    # 20 years: a new certificate changes the designated requirement, and every
    # install then asks once more for each Keychain item
    "$OPENSSL" req -x509 -newkey rsa:3072 -nodes -days 7300 -sha256 \
      -config "$WORK/cert.cnf" -keyout "$WORK/key.pem" -out "$WORK/cert.pem" 2>/dev/null
    PASSWORD="$("$OPENSSL" rand -hex 24)"
    (umask 077
      "$OPENSSL" pkcs12 -export -name "$NAME" -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
        -passout "pass:$PASSWORD" -out "$DIR/corvane-signing.p12"
      printf '%s\n' "$PASSWORD" > "$DIR/corvane-signing.password")
    import_p12 "$DIR/corvane-signing.p12" "$PASSWORD" "$KEYCHAIN"
    cat <<EOF
imported "$NAME" into $KEYCHAIN
saved $DIR/corvane-signing.p12 and $DIR/corvane-signing.password

Keep both in the password manager: releases must keep this certificate.
The first codesign run asks to use the key: click Always Allow.
For CI, add two repository secrets:
  MACOS_SIGNING_P12           base64 < "$DIR/corvane-signing.p12" | pbcopy
  MACOS_SIGNING_P12_PASSWORD  pbcopy < "$DIR/corvane-signing.password"
EOF
    ;;

  import)
    P12="${2:?usage: signing-cert.sh import <p12>}"
    PASSWORD="${MACOS_SIGNING_P12_PASSWORD:-}"
    if [[ -z "$PASSWORD" ]]; then
      if [[ -f "${P12%.p12}.password" ]]; then
        PASSWORD="$(cat "${P12%.p12}.password")"
      else
        read -rsp "password for $P12: " PASSWORD && echo
      fi
    fi
    import_p12 "$P12" "$PASSWORD" "$KEYCHAIN"
    echo "imported \"$NAME\" into $KEYCHAIN"
    ;;

  ci)
    : "${MACOS_SIGNING_P12:?MACOS_SIGNING_P12 (base64 .p12) is not set}"
    : "${MACOS_SIGNING_P12_PASSWORD:?MACOS_SIGNING_P12_PASSWORD is not set}"
    TMP="${RUNNER_TEMP:-$(mktemp -d)}"
    KC="$TMP/corvane-signing.keychain-db"
    KC_PASSWORD="$("$OPENSSL" rand -hex 24)"
    (umask 077; printf '%s' "$MACOS_SIGNING_P12" | base64 --decode > "$TMP/corvane-signing.p12")
    security create-keychain -p "$KC_PASSWORD" "$KC"
    security set-keychain-settings "$KC" # no auto-lock
    security unlock-keychain -p "$KC_PASSWORD" "$KC"
    import_p12 "$TMP/corvane-signing.p12" "$MACOS_SIGNING_P12_PASSWORD" "$KC"
    rm -f "$TMP/corvane-signing.p12"
    # codesign may use the key without a (headless, unanswerable) prompt
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KC_PASSWORD" "$KC" >/dev/null
    # keep the runner's own keychains searchable
    # shellcheck disable=SC2046
    security list-keychains -d user -s "$KC" $(security list-keychains -d user | tr -d '"')
    has_identity "$KC" || { echo "\"$NAME\" is not in the imported .p12" >&2; exit 1; }
    echo "imported \"$NAME\" into $KC"
    ;;

  *)
    sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
