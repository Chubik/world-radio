# World Radio Android (Kotlin) MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A native Kotlin Android app that streams a radio station, plays in the background with steering-wheel / lock-screen controls (play / stop / next=shuffle), runnable on the existing emulator and delivered to the phone via Firebase App Distribution.

**Architecture:** Kotlin + Media3 (ExoPlayer + MediaSession). A `MediaSessionService` owns the player and surfaces transport controls to the lock screen, Bluetooth/steering wheel, and Android Auto for free — the battery-light path (no UI engine resident when backgrounded). Stations come from the radio-browser API over HTTP; `next` shuffles a new station. One minimal Compose Activity renders now-playing.

**Tech Stack:** Kotlin, Gradle wrapper, AGP 8.5, Media3 1.4, OkHttp + kotlinx-serialization, Jetpack Compose (minimal), Firebase App Distribution.

## Global Constraints

- App lives at `android/` in the repo root, outside the Cargo workspace; does not touch the Rust crates.
- **Language: Kotlin** (battery is the #1 priority — native MediaSessionService, no UI engine in background). `preferans` is the reference only for *how Firebase App Distribution is wired*, not the language.
- Package id / applicationId: `net.vchub.r4dio` (everything ships under vchub.net, like `net.vchub.preferans`). App display name: `World Radio`.
- **Use the already-installed Android SDK at `~/android` — do not download SDK packages.** Available: `compileSdk=34` (platforms `android-34`, build-tools `34.0.0`), cmdline-tools/latest, emulator with system-images android-34-ext12/35/36. No NDK (Media3 doesn't need it).
- **Do NOT install a JDK and do NOT bootstrap Gradle.** Use the team pattern from preferans/subtick: JDK comes from Android Studio's bundled runtime (`JAVA_HOME=/Applications/Android Studio.app/Contents/jbr/Contents/Home`, confirmed present), and the Gradle wrapper (`gradlew` + `gradle/wrapper/gradle-wrapper.jar` + properties) is **copied from `~/dev/projects/preferans/mobile_flutter/android/`** (gradle 8.14), never generated. Java compat is pinned to 17 in the gradle files (`JavaVersion.VERSION_17`, `jvmTarget = 17`) — the proven value; do not experiment.
- minSdk 26 (Android Auto + media session). targetSdk 34.
- Steering-wheel/lock-screen controls via MediaSession standard commands: play/pause → player; next → shuffle a new station; previous → previous station (or shuffle if none).
- Catalog source: `https://all.api.radio-browser.info/json/stations/search` (same as desktop).
- Delivery: Firebase App Distribution to the phone (tester group), via the preferans-style `firebase appdistribution:distribute --app <APP_ID> --groups <group>` pattern. NOT Google Play.
- Battery: foreground media service only, no polling loops, no visualizer.
- Logs/strings English. No personal/AI mentions anywhere.
- Commit to `dev`; messages English, concise, no AI/personal mentions.
- On-device audio + wheel controls are not unit-testable — those steps are manual on-device/emulator smoke tests the human runs; the implementer runs JVM unit tests and `./gradlew assembleDebug`.

---

### Task 1: Gradle project skeleton that builds an empty app

**Files:**
- Create: `android/settings.gradle.kts`, `android/build.gradle.kts`, `android/gradle.properties`, `android/local.properties`
- Create: `android/app/build.gradle.kts`
- Create: `android/app/src/main/AndroidManifest.xml`
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt`
- Create: `android/app/src/main/res/values/strings.xml`
- Create: `android/.gitignore`
- Copy (from preferans): `android/gradlew` + `android/gradle/wrapper/{gradle-wrapper.jar,gradle-wrapper.properties}`
- Modify: `Makefile` (repo root — add android targets)

**Interfaces:**
- Produces: a buildable app (`./gradlew assembleDebug` succeeds) with one empty `MainActivity`, plus `make android-emu` / `android-run` / `android-build` targets.

- [ ] **Step 1: Create `android/.gitignore`**

```gitignore
.gradle/
build/
local.properties
.idea/
*.iml
app/google-services.json
release-notes.txt
```

- [ ] **Step 2: Create `android/local.properties`** (points Gradle at the installed SDK — gitignored)

```properties
sdk.dir=/Users/vchub/android
```

- [ ] **Step 3: Create `android/gradle.properties`**

```properties
org.gradle.jvmargs=-Xmx2048m
android.useAndroidX=true
kotlin.code.style=official
```

- [ ] **Step 4: Create `android/settings.gradle.kts`**

```kotlin
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "World Radio"
include(":app")
```

- [ ] **Step 5: Create `android/build.gradle.kts`** (root)

```kotlin
plugins {
    id("com.android.application") version "8.5.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.20" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.0.20" apply false
}
```

- [ ] **Step 6: Create `android/app/build.gradle.kts`**

```kotlin
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.serialization")
}

android {
    namespace = "net.vchub.r4dio"
    compileSdk = 34

    defaultConfig {
        applicationId = "net.vchub.r4dio"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "1.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        compose = true
    }
    composeOptions {
        kotlinCompilerExtensionVersion = "1.5.14"
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.2")
    implementation(platform("androidx.compose:compose-bom:2024.09.02"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.media3:media3-exoplayer:1.4.1")
    implementation("androidx.media3:media3-session:1.4.1")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.1")
    testImplementation("junit:junit:4.13.2")
}
```

- [ ] **Step 7: Create `android/app/src/main/res/values/strings.xml`**

```xml
<resources>
    <string name="app_name">World Radio</string>
</resources>
```

- [ ] **Step 8: Create `android/app/src/main/AndroidManifest.xml`**

```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />

    <application
        android:label="@string/app_name"
        android:theme="@android:style/Theme.Material.NoActionBar">
        <activity
            android:name=".MainActivity"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
```

- [ ] **Step 9: Create `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt`**

```kotlin
package net.vchub.r4dio

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.Text

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            Text("World Radio")
        }
    }
}
```

- [ ] **Step 10: Copy the Gradle wrapper from preferans (do NOT generate it)**

The team already has a working wrapper. Copy it verbatim into `android/`:
```bash
mkdir -p android/gradle/wrapper
cp ~/dev/projects/preferans/mobile_flutter/android/gradlew android/gradlew
cp ~/dev/projects/preferans/mobile_flutter/android/gradlew.bat android/gradlew.bat 2>/dev/null || true
cp ~/dev/projects/preferans/mobile_flutter/android/gradle/wrapper/gradle-wrapper.jar android/gradle/wrapper/gradle-wrapper.jar
cp ~/dev/projects/preferans/mobile_flutter/android/gradle/wrapper/gradle-wrapper.properties android/gradle/wrapper/gradle-wrapper.properties
chmod +x android/gradlew
```
This brings gradle 8.14 (the version preferans builds with). Do not download or bootstrap Gradle.

- [ ] **Step 11: (removed — wrapper is copied in Step 10, not generated)**

Nothing to do here; proceed to Step 12.

- [ ] **Step 12: Add Makefile android targets** — create/append `Makefile` at the repo root:

```makefile
ANDROID_HOME ?= $(HOME)/android
JAVA_HOME    ?= /Applications/Android Studio.app/Contents/jbr/Contents/Home
export JAVA_HOME
EMULATOR_ID ?= $(shell $(ANDROID_HOME)/emulator/emulator -list-avds 2>/dev/null | head -1)
ADB := $(ANDROID_HOME)/platform-tools/adb

.PHONY: android-emu android-build android-run android-install

android-emu:
	@if $(ADB) shell getprop sys.boot_completed 2>/dev/null | grep -q 1; then \
	  echo "emulator already booted"; \
	else \
	  $(ANDROID_HOME)/emulator/emulator -avd $(EMULATOR_ID) -netdelay none -netspeed full > /dev/null 2>&1 & \
	  echo "booting $(EMULATOR_ID) (60-120s)..."; \
	  $(ADB) wait-for-device; \
	  until $(ADB) shell getprop sys.boot_completed 2>/dev/null | grep -q 1; do sleep 2; done; \
	  echo "emulator ready"; \
	fi

android-build:
	cd android && ANDROID_HOME=$(ANDROID_HOME) ./gradlew assembleDebug

android-install: android-build android-emu
	$(ADB) install -r android/app/build/outputs/apk/debug/app-debug.apk

android-run: android-install
	$(ADB) shell monkey -p net.vchub.r4dio -c android.intent.category.LAUNCHER 1
```

- [ ] **Step 13: Build the empty app** (using Android Studio's JDK, not a system one)

Run:
```bash
cd android && \
  JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" \
  ANDROID_HOME=$HOME/android ./gradlew assembleDebug
```
Expected: `BUILD SUCCESSFUL`; APK at `android/app/build/outputs/apk/debug/app-debug.apk`. First run downloads AGP/Kotlin/Compose Gradle plugins (NOT the SDK, NOT Gradle itself — the wrapper was copied) — may take minutes. If a Gradle/AGP-vs-JDK error appears, confirm `JAVA_HOME` points at the Android Studio jbr (present) — do not install another JDK; report the exact error.

- [ ] **Step 14: Commit**

```bash
git add android Makefile
git commit -m "chore: scaffold android kotlin app project"
```

---

### Task 2: Catalog client + shuffle pick (radio-browser, unit-tested)

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/Station.kt`
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt`
- Create: `android/app/src/test/kotlin/net/vchub/r4dio/ShuffleTest.kt`

**Interfaces:**
- Produces:
  - `data class Station(val uuid, name, url, country, codec: String, val bitrate: Int)`
  - `fun pickRandom(stations: List<Station>, rng: Random = Random.Default): Station?` — random station with non-blank url, or null.
  - `class Catalog(client: OkHttpClient = OkHttpClient())` with `fun fetchStations(limit: Int = 200): List<Station>`.

- [ ] **Step 1: Create `Station.kt`**

```kotlin
package net.vchub.r4dio

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ApiStation(
    val stationuuid: String = "",
    val name: String = "",
    @SerialName("url_resolved") val urlResolved: String = "",
    val countrycode: String = "",
    val codec: String = "",
    val bitrate: Int = 0,
)

data class Station(
    val uuid: String,
    val name: String,
    val url: String,
    val country: String,
    val codec: String,
    val bitrate: Int,
)

fun ApiStation.toStation(): Station =
    Station(stationuuid, name, urlResolved, countrycode, codec, bitrate)
```

- [ ] **Step 2: Write the failing test** — create `android/app/src/test/kotlin/net/vchub/r4dio/ShuffleTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

class ShuffleTest {
    private fun st(uuid: String, url: String) = Station(uuid, uuid, url, "", "", 0)

    @Test
    fun pickRandom_returnsNull_forEmpty() {
        assertNull(pickRandom(emptyList()))
    }

    @Test
    fun pickRandom_skipsStationsWithBlankUrl() {
        val list = listOf(st("a", ""), st("b", "http://x"))
        assertEquals("b", pickRandom(list, Random(1))?.uuid)
    }

    @Test
    fun pickRandom_returnsAPlayableOne() {
        val list = listOf(st("a", "http://a"), st("b", "http://b"))
        val p = pickRandom(list, Random(42))!!
        assertTrue(p.uuid == "a" || p.uuid == "b")
        assertTrue(p.url.isNotBlank())
    }
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd android && ANDROID_HOME=$HOME/android ./gradlew testDebugUnitTest --tests "net.vchub.r4dio.ShuffleTest"`
Expected: FAIL — `pickRandom` unresolved.

- [ ] **Step 4: Create `Catalog.kt`**

```kotlin
package net.vchub.r4dio

import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request
import kotlin.random.Random

fun pickRandom(stations: List<Station>, rng: Random = Random.Default): Station? {
    val playable = stations.filter { it.url.isNotBlank() }
    if (playable.isEmpty()) return null
    return playable[rng.nextInt(playable.size)]
}

class Catalog(private val client: OkHttpClient = OkHttpClient()) {
    private val json = Json { ignoreUnknownKeys = true }

    fun fetchStations(limit: Int = 200): List<Station> {
        val url =
            "https://all.api.radio-browser.info/json/stations/search" +
                "?limit=$limit&hidebroken=true&order=clickcount&reverse=true"
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        client.newCall(request).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful || body.isBlank()) return emptyList()
            val api = json.decodeFromString<List<ApiStation>>(body)
            return api.map { it.toStation() }.filter { it.url.isNotBlank() }
        }
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd android && ANDROID_HOME=$HOME/android ./gradlew testDebugUnitTest --tests "net.vchub.r4dio.ShuffleTest"`
Expected: PASS — 3 tests.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/Station.kt android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt android/app/src/test
git commit -m "feat: android catalog client and shuffle pick"
```

---

### Task 3: MediaSessionService with ExoPlayer + shuffle on next

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
- Modify: `android/app/src/main/AndroidManifest.xml`
- Modify: `android/app/build.gradle.kts` (add media3-common if a symbol needs it)

**Interfaces:**
- Consumes: `Catalog`, `pickRandom`, `Station` (Task 2).
- Produces: `class PlaybackService : MediaSessionService` exposing an ExoPlayer-backed `MediaSession`; a `ForwardingPlayer` maps next/previous to shuffling a new station. Runs as a foreground media service so lock-screen / Bluetooth / wheel controls work.

- [ ] **Step 1: Add permissions + service to `AndroidManifest.xml`** — inside `<manifest>`:

```xml
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK" />
```

inside `<application>`:

```xml
        <service
            android:name=".PlaybackService"
            android:foregroundServiceType="mediaPlayback"
            android:exported="true">
            <intent-filter>
                <action android:name="androidx.media3.session.MediaSessionService" />
            </intent-filter>
        </service>
```

- [ ] **Step 2: Create `PlaybackService.kt`**

```kotlin
package net.vchub.r4dio

import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import kotlin.concurrent.thread

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private var exo: ExoPlayer? = null
    private val catalog = Catalog()
    @Volatile private var stations: List<Station> = emptyList()

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this).build()
        exo = player
        val forwarding = object : ForwardingPlayer(player) {
            override fun seekToNext() = shuffle()
            override fun seekToNextMediaItem() = shuffle()
            override fun hasNextMediaItem() = true
        }
        session = MediaSession.Builder(this, forwarding).build()
        loadStations()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        session?.release()
        exo?.release()
        session = null
        exo = null
        super.onDestroy()
    }

    private fun loadStations() {
        thread {
            stations = catalog.fetchStations()
        }
    }

    private fun shuffle() {
        val pick = pickRandom(stations) ?: return
        val player = exo ?: return
        player.setMediaItem(MediaItem.fromUri(pick.url))
        player.prepare()
        player.play()
    }
}
```

- [ ] **Step 3: Build**

Run: `cd android && ANDROID_HOME=$HOME/android ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL. If `ForwardingPlayer` is unresolved, add `implementation("androidx.media3:media3-common:1.4.1")` to `app/build.gradle.kts` and rebuild. Report any unresolved Media3 symbol with its exact name.

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt android/app/src/main/AndroidManifest.xml android/app/build.gradle.kts
git commit -m "feat: media3 playback service, next shuffles a station"
```

---

### Task 4: UI — connect the Activity to the service (play / stop / shuffle)

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt`
- Modify: `android/app/build.gradle.kts` (guava if `MoreExecutors` is needed)

**Interfaces:**
- Consumes: `PlaybackService` (Task 3) via a `MediaController`.
- Produces: a minimal amber Compose screen — current station, ⇄ SHUFFLE, ▶/⏸ — driven by a `MediaController` bound to the session.

- [ ] **Step 1: Replace `MainActivity.kt`**

```kotlin
package net.vchub.r4dio

import android.content.ComponentName
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors

class MainActivity : ComponentActivity() {
    private var controller: MediaController? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        val future = MediaController.Builder(this, token).buildAsync()

        setContent {
            var isPlaying by remember { mutableStateOf(false) }
            var title by remember { mutableStateOf("Nothing playing") }

            future.addListener({
                val c = future.get()
                controller = c
                c.addListener(object : Player.Listener {
                    override fun onIsPlayingChanged(playing: Boolean) {
                        isPlaying = playing
                    }
                    override fun onMediaMetadataChanged(m: MediaMetadata) {
                        title = m.title?.toString() ?: "World Radio"
                    }
                })
            }, MoreExecutors.directExecutor())

            Column(
                modifier = Modifier.fillMaxSize().background(Color(0xFF15100B)).padding(24.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(text = "▌ r4dio", color = Color(0xFFFFC457))
                Spacer(Modifier.height(8.dp))
                Text(text = title, color = Color(0xFFFFF0C0))
                Spacer(Modifier.height(24.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Button(onClick = { controller?.seekToNext() }) { Text("⇄ SHUFFLE") }
                    Button(onClick = {
                        val c = controller ?: return@Button
                        if (c.isPlaying) c.pause() else c.play()
                    }) { Text(if (isPlaying) "⏸" else "▶") }
                }
            }
        }
    }

    override fun onDestroy() {
        controller?.release()
        controller = null
        super.onDestroy()
    }
}
```

- [ ] **Step 2: Add guava if needed** — to `app/build.gradle.kts` `dependencies`:

```kotlin
    implementation("com.google.guava:guava:33.3.0-android")
```

- [ ] **Step 3: Build**

Run: `cd android && ANDROID_HOME=$HOME/android ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL. If `MoreExecutors` resolves via media3-session transitively, remove the guava line; report which case held.

- [ ] **Step 4: MANUAL smoke test (emulator + phone)** — human runs:

`make android-run` (boots the emulator and launches the app). Expected: app opens; SHUFFLE starts a station within a few seconds; ▶/⏸ toggles; a media notification appears; locking the screen keeps audio playing; the notification's play/pause/next work; on a real phone, Bluetooth/steering-wheel play/pause/next control playback.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt android/app/build.gradle.kts
git commit -m "feat: android ui wired to playback session"
```

---

### Task 5: Firebase App Distribution (to the phone)

**Files:**
- Modify: `Makefile` (add distribute target)
- Create: `android/README.md`

**Interfaces:**
- Consumes: the release APK (`./gradlew assembleRelease`).
- Produces: `make android-distribute` uploading the APK to Firebase App Distribution, mirroring the preferans `appdistribution:distribute --app <id> --groups` pattern.

- [ ] **Step 1: Create the Firebase Android app**

The Firebase CLI is authenticated. Create (or reuse) a project for radio:
```bash
firebase projects:create r4dio-vchub --display-name "World Radio"
firebase apps:create ANDROID "World Radio" --package-name net.vchub.r4dio --project r4dio-vchub
firebase apps:sdkconfig ANDROID --project r4dio-vchub > android/app/google-services.json
```
Record the printed **App ID** (`1:NNNN:android:XXXX`) — the distribute target needs it. If `projects:create` is denied, report the exact error; the user creates the project in the console and gives you the project id + app id. `google-services.json` is gitignored (Task 1) — do not commit it.

- [ ] **Step 2: Add the distribute target to `Makefile`** (fill in the real App ID from Step 1)

```makefile
RADIO_APP_ID ?= <APP_ID_FROM_STEP_1>

.PHONY: android-distribute
android-distribute:
	cd android && ANDROID_HOME=$(ANDROID_HOME) ./gradlew assembleRelease
	printf "World Radio test build\n\nRecent changes:\n%s\n" "$$(git log -5 --pretty=format:'- %s')" > android/release-notes.txt
	firebase appdistribution:distribute \
	  android/app/build/outputs/apk/release/app-release.apk \
	  --app $(RADIO_APP_ID) \
	  --groups internal \
	  --release-notes-file android/release-notes.txt
```

- [ ] **Step 3: Create the tester group + add the user**

```bash
firebase appdistribution:group:create internal "Internal" --project r4dio-vchub
firebase appdistribution:testers:add <user-email> --project r4dio-vchub
```
Use the user's email. Report if these need console setup.

- [ ] **Step 4: Build + distribute**

Run: `make android-distribute`
Expected: a release APK builds; the CLI prints an uploaded release URL; the tester gets an install link. Report the release URL. (Release build is unsigned-for-store but App Distribution accepts the debug-signed/auto-signed APK for testers; if `assembleRelease` fails for missing signing config, fall back to `assembleDebug` + distribute the debug APK and note it.)

- [ ] **Step 5: Create `android/README.md`**

```markdown
# World Radio — Android (Kotlin)

Native Kotlin radio player (Media3). Background playback with lock-screen / steering-wheel
controls; stations from radio-browser.

## Run on emulator

    make android-run

## Distribute (Firebase App Distribution)

    make android-distribute

Sends the APK to the `internal` tester group. `google-services.json` is not committed.
```

- [ ] **Step 6: Commit** (no `google-services.json`, no `release-notes.txt`)

```bash
git add Makefile android/README.md
git commit -m "chore: firebase app distribution for android"
```

---

## Self-Review Notes

- **Spec coverage:** Gradle scaffold using installed SDK + Makefile (Task 1) · Kotlin catalog client + shuffle, unit-tested mirroring desktop `pick_random` (Task 2) · Media3 `MediaSessionService` + ExoPlayer, next=shuffle, foreground media service for lock-screen/Bluetooth/wheel (Task 3) · minimal amber Compose UI via MediaController (Task 4) · Firebase App Distribution to the phone (Task 5). Battery: foreground media service, no polling, no visualizer.
- **User corrections applied:** Kotlin (not Flutter) for battery; package `net.vchub.r4dio` under vchub.net; use the installed `~/android` SDK (compileSdk 34, no downloads); Java pinned to 17 (preferans's proven value, not re-litigated); Firebase wired the preferans way (`appdistribution:distribute --app <id> --groups`); preferans is reference-only.
- **Out of scope (per spec):** favorites/sync, search UI, offline cache, Crashlytics/Analytics, Google Play, iOS, Rust reuse, live spectrum — none planned.
- **Type consistency:** `Station(uuid,name,url,country,codec,bitrate)` (Task 2) consumed by `pickRandom` (Task 2) and `PlaybackService` (Task 3); `pickRandom(List<Station>, Random): Station?` consistent; `PlaybackService` + `SessionToken(ComponentName(.., PlaybackService))` consistent Task 3 → Task 4; UI `seekToNext()` maps to the `ForwardingPlayer.seekToNext` override (Task 3) calling `shuffle()`.
- **Known risks (called out in tasks):** (1) Gradle wrapper jar bootstrap in a headless env (Task 1 Step 11 — BLOCKED path documented); (2) Media3 `ForwardingPlayer` artifact (Task 3 Step 3 — add media3-common, report exact symbol); (3) `MoreExecutors`/guava transitive availability (Task 4 Step 3); (4) `firebase projects:create` permission (Task 5 Step 1 — console fallback); (5) release signing (Task 5 Step 4 — debug-APK fallback). Each names the exact failure to report.
- **Manual gates:** on-device/emulator audio + wheel controls (Task 4 Step 4) and real distribution (Task 5 Step 4) are human-run.
```
