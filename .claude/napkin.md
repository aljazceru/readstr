# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|

## User Preferences
- Use the local Android/ADB toolchain directly for build-and-install requests.

## Patterns That Work
- `cd android && ./gradlew assembleDebug` produces the installable debug APK for this repo.
- If the host default JDK is 25, build with `JAVA_HOME=/usr/lib/jvm/java-21` because Gradle/Kotlin DSL in this repo fails during configuration on Java 25.0.2.
- Full USB deploy path that worked: `export JAVA_HOME=/usr/lib/jvm/java-21 && cd android && ./gradlew assembleDebug` then `adb install -r android/app/build/outputs/apk/debug/app-debug.apk`.

## Patterns That Don't Work
- Building with the system default OpenJDK 25.0.2 fails early with `java.lang.IllegalArgumentException: 25.0.2` from Kotlin/Gradle.

## Domain Notes
- Android app id is `dev.disobey.speedreadingapp`; debug builds use suffix `.dev`.
- USB-connected device installs can be handled with `adb install -r android/app/build/outputs/apk/debug/app-debug.apk`.
