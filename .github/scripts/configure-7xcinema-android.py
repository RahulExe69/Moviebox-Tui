#!/usr/bin/env python3
from pathlib import Path
import re
import shutil
import sys


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


ROOT = Path(__file__).resolve().parents[2]
ANDROID = ROOT / "mobile" / "android"
LIBRARY = ROOT / "target" / "aarch64-linux-android" / "release" / "libmoviebox_tui.so"
JAVA_SOURCE = ROOT / "android-local-backend"
JAVA_DEST = ANDROID / "app" / "src" / "main" / "java" / "com" / "sevenx" / "cinema"

if not ANDROID.is_dir():
    fail("Capacitor Android project was not generated at mobile/android.")
if not LIBRARY.is_file():
    fail("Rust ARM64 shared library is missing: " + str(LIBRARY))

JAVA_DEST.mkdir(parents=True, exist_ok=True)
for filename in ("MobileBackendPlugin.java", "MainActivity.java", "PlayerActivity.java"):
    source = JAVA_SOURCE / filename
    if not source.is_file():
        fail("Required native Java source is missing: " + str(source))
    shutil.copy2(source, JAVA_DEST / filename)

jni_libs = ANDROID / "app" / "src" / "main" / "jniLibs" / "arm64-v8a"
jni_libs.mkdir(parents=True, exist_ok=True)
shutil.copy2(LIBRARY, jni_libs / "libmoviebox_tui.so")

manifest_path = ANDROID / "app" / "src" / "main" / "AndroidManifest.xml"
if not manifest_path.is_file():
    fail("Generated AndroidManifest.xml was not found.")
manifest = manifest_path.read_text(encoding="utf-8")

# Capacitor generates a generic launcher icon each time the Android project is
# scaffolded. Replace it with the original 7xCinema cinema-gold/teal mark.
manifest, icon_count = re.subn(
    r'(android:(?:roundIcon|icon)\s*=\s*)"@mipmap/[^"]+"',
    r'\1"@drawable/ic_7xcinema_launcher"',
    manifest,
)
if icon_count == 0 and 'android:icon="@drawable/ic_7xcinema_launcher"' not in manifest:
    fail("Could not replace the generated Android launcher icon reference.")

drawable_dir = ANDROID / "app" / "src" / "main" / "res" / "drawable"
drawable_dir.mkdir(parents=True, exist_ok=True)
launcher_icon = drawable_dir / "ic_7xcinema_launcher.xml"
launcher_icon.write_text(
    """<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="128"
    android:viewportHeight="128">
    <path android:fillColor="#080C15"
        android:pathData="M24,5 H104 A19,19 0,0 1 123,24 V104 A19,19 0,0 1 104,123 H24 A19,19 0,0 1 5,104 V24 A19,19 0,0 1 24,5 Z" />
    <path android:fillColor="#101A2B" android:strokeColor="#F6BF58" android:strokeWidth="3"
        android:pathData="M28,25 H100 V91 H28 Z" />
    <path android:fillColor="#55D9D1" android:pathData="M17,29 H23 V87 H17 Z M105,29 H111 V87 H105 Z" />
    <path android:fillColor="#FFE1A6" android:pathData="M49,39 L84,58 L49,77 Z" />
    <path android:fillColor="#F6BF58" android:pathData="M28,99 H100 V103 H28 Z" />
    <path android:fillColor="#55D9D1" android:pathData="M35,108 H52 V112 H35 Z M57,108 H74 V112 H57 Z M79,108 H96 V112 H79 Z" />
</vector>
""",
    encoding="utf-8",
)
if "android.permission.INTERNET" not in manifest:
    manifest = manifest.replace(
        "<application",
        '    <uses-permission android:name="android.permission.INTERNET" />\n\n    <application',
        1,
    )
if not re.search(r'android:usesCleartextTraffic\s*=', manifest):
    manifest = manifest.replace("<application", '<application android:usesCleartextTraffic="true"', 1)
if 'android:name=".PlayerActivity"' not in manifest:
    manifest = manifest.replace(
        "</application>",
        '        <activity android:name=".PlayerActivity" '
        'android:exported="false" android:screenOrientation="sensorLandscape" '
        'android:configChanges="orientation|screenSize|keyboardHidden" />\n    </application>',
        1,
    )
manifest_path.write_text(manifest, encoding="utf-8")

gradle_path = ANDROID / "app" / "build.gradle"
if not gradle_path.is_file():
    fail("Generated app/build.gradle was not found.")
gradle = gradle_path.read_text(encoding="utf-8")
if "abiFilters" not in gradle:
    gradle, count = re.subn(
        r"(defaultConfig\s*\{)",
        r'\1\n        ndk { abiFilters "arm64-v8a" }',
        gradle,
        count=1,
    )
    if count != 1:
        fail("Could not configure the ARM64 ABI filter in app/build.gradle.")

media3_dependencies = """    implementation "androidx.media3:media3-exoplayer:1.11.1"
    implementation "androidx.media3:media3-exoplayer-hls:1.11.1"
    implementation "androidx.media3:media3-exoplayer-dash:1.11.1"
    implementation "androidx.media3:media3-ui:1.11.1"
"""
if "androidx.media3:media3-exoplayer:" not in gradle:
    gradle, count = re.subn(
        r"(?m)^\s*dependencies\s*\{",
        lambda match: match.group(0) + "\n" + media3_dependencies,
        gradle,
        count=1,
    )
    if count != 1:
        fail("Could not add AndroidX Media3 dependencies to app/build.gradle.")

gradle_path.write_text(gradle, encoding="utf-8")

print("Configured 7xCinema native bridge, branded launcher icon, and libmoviebox_tui.so for arm64-v8a.")
