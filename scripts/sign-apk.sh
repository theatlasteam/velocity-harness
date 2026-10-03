#!/bin/sh
set -eu
: "${ANDROID_HOME:?Set ANDROID_HOME}"
INPUT=${1:?Unsigned APK}; OUTPUT=${2:-Velocity-Harness-android-arm64.apk}
BUILD_TOOLS=$(find "$ANDROID_HOME/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)
KEYSTORE=${VELOCITY_KEYSTORE:-"$HOME/.velocity-harness/signing.p12"}
mkdir -p "$(dirname "$KEYSTORE")"
if [ ! -f "$KEYSTORE" ]; then
  # Development/distribution signing, NOT a Play Store production identity.
  keytool -genkeypair -keystore "$KEYSTORE" -storetype PKCS12 -alias velocity -keyalg RSA -keysize 3072 -validity 3650 -storepass velocity-development -keypass velocity-development -dname "CN=Velocity Harness Development,O=Velocity Harness,C=US"
  chmod 600 "$KEYSTORE"
fi
"$BUILD_TOOLS/zipalign" -p -f 4 "$INPUT" "$OUTPUT.aligned"
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-key-alias velocity --ks-pass pass:velocity-development --key-pass pass:velocity-development --out "$OUTPUT" "$OUTPUT.aligned"
rm "$OUTPUT.aligned"
"$BUILD_TOOLS/apksigner" verify --verbose "$OUTPUT"
