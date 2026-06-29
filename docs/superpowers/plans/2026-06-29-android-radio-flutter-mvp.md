# World Radio Android (Flutter) MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Flutter Android app that streams a radio station, plays in the background with steering-wheel / lock-screen controls (play / stop / next=shuffle), runnable on the existing emulator and delivered to the phone via Firebase App Distribution — reusing the `preferans` mobile infrastructure.

**Architecture:** Flutter (Dart) with `just_audio` for streaming and `audio_service` for the background service + Android MediaSession (which gives lock-screen / Bluetooth / steering-wheel transport for free). Stations come from the radio-browser API over HTTP; `next` shuffles a new station. One minimal screen renders now-playing. Build/run/distribute via Makefile targets copied from `preferans`.

**Tech Stack:** Flutter/Dart, just_audio, audio_service, http, Firebase App Distribution. Mirrors `~/dev/projects/preferans/mobile_flutter`.

## Global Constraints

- The Flutter app lives at `mobile_flutter/` in the repo root (same layout as preferans), outside the Cargo workspace; it does not touch the Rust crates.
- Reuse the preferans Makefile mobile pattern: targets `mobile-flutter-emu`, `mobile-flutter-app`, `mobile-flutter-build-debug`, `mobile-flutter-build-release`, plus an `apk`/distribute target. Copy them; adapt names/paths to this repo.
- **Do not pin or fiddle with the Java/JDK version** — the preferans Flutter setup already resolves it; let Flutter's Gradle handle it. If a build fails on toolchain, copy preferans's `android/` Gradle config rather than inventing versions.
- App display name: `World Radio`. Android applicationId: `net.r4dio.radio`.
- Logs/strings English. No personal/AI mentions anywhere.
- Steering-wheel/lock-screen controls via `audio_service` standard handlers: play/pause → player; skipToNext → shuffle a new station; skipToPrevious → previous station (or shuffle if none).
- Catalog source: `https://all.api.radio-browser.info/json/stations/search` (same as desktop).
- Delivery: Firebase App Distribution to the phone (tester group), NOT Google Play.
- Emulator already exists (`Pixel_7`, device `emulator-5554`) and boots via the Makefile target.
- Commit to `dev`; messages English, concise, no AI/personal mentions.
- On-device audio + wheel controls are not unit-testable — those steps are manual on-device/emulator smoke tests the human runs; the implementer runs `flutter analyze`, `flutter test`, and `flutter build apk --debug`.

---

### Task 1: Flutter project scaffold that builds + Makefile targets

**Files:**
- Create: `mobile_flutter/` (via `flutter create`)
- Modify: `mobile_flutter/pubspec.yaml`
- Replace: `mobile_flutter/lib/main.dart`
- Create: `Makefile` (repo root — add mobile-flutter targets)
- Create: `mobile_flutter/.gitignore` (flutter default includes it)

**Interfaces:**
- Produces: a Flutter app that runs on the emulator showing an empty "World Radio" screen, plus `make mobile-flutter-emu` / `mobile-flutter-app` / `mobile-flutter-build-debug` targets.

- [ ] **Step 1: Scaffold the Flutter app**

From the repo root:
```bash
flutter create --org net.r4dio --project-name world_radio --platforms android mobile_flutter
```
This creates `mobile_flutter/` with the standard Android Gradle setup (the same kind preferans uses — let it pick the toolchain).

- [ ] **Step 2: Set applicationId + label** — in `mobile_flutter/android/app/build.gradle.kts` set `applicationId = "net.r4dio.radio"`; in `mobile_flutter/android/app/src/main/AndroidManifest.xml` set `android:label="World Radio"`.

- [ ] **Step 3: Add dependencies to `mobile_flutter/pubspec.yaml`** under `dependencies:`:

```yaml
  http: ^1.2.2
  just_audio: ^0.9.42
  audio_service: ^0.18.15
```

Run `cd mobile_flutter && flutter pub get`.

- [ ] **Step 4: Replace `mobile_flutter/lib/main.dart`** with a minimal app

```dart
import 'package:flutter/material.dart';

void main() => runApp(const WorldRadioApp());

class WorldRadioApp extends StatelessWidget {
  const WorldRadioApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'World Radio',
      theme: ThemeData(scaffoldBackgroundColor: const Color(0xFF15100B)),
      home: const Scaffold(
        body: Center(
          child: Text('World Radio', style: TextStyle(color: Color(0xFFFFC457))),
        ),
      ),
    );
  }
}
```

- [ ] **Step 5: Add Makefile mobile targets** — create `Makefile` at the repo root (or append if one exists) with the preferans-style targets, adapted:

```makefile
EMULATOR_ID ?= Pixel_7
DEVICE ?= emulator-5554

.PHONY: mobile-flutter-clean mobile-flutter-analyze mobile-flutter-test \
        mobile-flutter-emu mobile-flutter-app \
        mobile-flutter-build-debug mobile-flutter-build-release

mobile-flutter-clean:
	cd mobile_flutter && flutter clean && flutter pub get

mobile-flutter-analyze:
	cd mobile_flutter && flutter analyze

mobile-flutter-test:
	cd mobile_flutter && flutter test

mobile-flutter-emu:
	@if adb shell getprop sys.boot_completed 2>/dev/null | grep -q 1; then \
	  echo "emulator already booted"; \
	else \
	  flutter emulators --launch $(EMULATOR_ID) > /dev/null 2>&1 & \
	  echo "booting $(EMULATOR_ID) (60-120s)..."; \
	  adb wait-for-device; \
	  until adb shell getprop sys.boot_completed 2>/dev/null | grep -q 1; do sleep 2; done; \
	  adb shell input keyevent 82 > /dev/null 2>&1 || true; \
	  echo "emulator ready"; \
	fi

mobile-flutter-app: mobile-flutter-emu
	cd mobile_flutter && flutter run -d $(DEVICE)

mobile-flutter-build-debug:
	cd mobile_flutter && flutter build apk --debug

mobile-flutter-build-release:
	cd mobile_flutter && flutter build apk --release
```

- [ ] **Step 6: Analyze + build**

Run: `cd mobile_flutter && flutter analyze && flutter build apk --debug`
Expected: analyze clean; `BUILD SUCCESSFUL`; APK at `mobile_flutter/build/app/outputs/flutter-apk/app-debug.apk`. First build downloads Gradle/AGP — may take minutes. If it fails on Android toolchain, copy the `android/app/build.gradle.kts` Gradle/Kotlin/compileSdk settings from `~/dev/projects/preferans/mobile_flutter/android/` (proven working) rather than changing versions ad hoc.

- [ ] **Step 7: Commit**

```bash
git add mobile_flutter Makefile
git commit -m "chore: scaffold flutter android app with make targets"
```

---

### Task 2: Catalog client + shuffle pick (radio-browser, unit-tested)

**Files:**
- Create: `mobile_flutter/lib/domain/station.dart`
- Create: `mobile_flutter/lib/data/catalog.dart`
- Create: `mobile_flutter/test/shuffle_test.dart`

**Interfaces:**
- Produces:
  - `class Station { final String uuid, name, url, country, codec; final int bitrate; ... Station.fromJson(Map) }`
  - `Station? pickRandom(List<Station> stations, [Random? rng])` — random station with non-blank url, or null.
  - `class Catalog { Future<List<Station>> fetchStations({int limit = 200}) }` hitting radio-browser.

- [ ] **Step 1: Create `mobile_flutter/lib/domain/station.dart`**

```dart
class Station {
  final String uuid;
  final String name;
  final String url;
  final String country;
  final String codec;
  final int bitrate;

  const Station({
    required this.uuid,
    required this.name,
    required this.url,
    required this.country,
    required this.codec,
    required this.bitrate,
  });

  factory Station.fromJson(Map<String, dynamic> j) => Station(
        uuid: (j['stationuuid'] ?? '') as String,
        name: (j['name'] ?? '') as String,
        url: (j['url_resolved'] ?? '') as String,
        country: (j['countrycode'] ?? '') as String,
        codec: (j['codec'] ?? '') as String,
        bitrate: (j['bitrate'] ?? 0) as int,
      );
}
```

- [ ] **Step 2: Write the failing test** — create `mobile_flutter/test/shuffle_test.dart`:

```dart
import 'dart:math';
import 'package:flutter_test/flutter_test.dart';
import 'package:world_radio/data/catalog.dart';
import 'package:world_radio/domain/station.dart';

Station st(String uuid, String url) =>
    Station(uuid: uuid, name: uuid, url: url, country: '', codec: '', bitrate: 0);

void main() {
  test('pickRandom returns null for empty', () {
    expect(pickRandom([]), isNull);
  });

  test('pickRandom skips stations with blank url', () {
    final list = [st('a', ''), st('b', 'http://x')];
    expect(pickRandom(list, Random(1))?.uuid, 'b');
  });

  test('pickRandom returns a playable one', () {
    final list = [st('a', 'http://a'), st('b', 'http://b')];
    final p = pickRandom(list, Random(42))!;
    expect(p.uuid == 'a' || p.uuid == 'b', isTrue);
    expect(p.url.isNotEmpty, isTrue);
  });
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd mobile_flutter && flutter test test/shuffle_test.dart`
Expected: FAIL — `catalog.dart` / `pickRandom` missing.

- [ ] **Step 4: Create `mobile_flutter/lib/data/catalog.dart`**

```dart
import 'dart:convert';
import 'dart:math';
import 'package:http/http.dart' as http;
import '../domain/station.dart';

Station? pickRandom(List<Station> stations, [Random? rng]) {
  final playable = stations.where((s) => s.url.isNotEmpty).toList();
  if (playable.isEmpty) return null;
  final r = rng ?? Random();
  return playable[r.nextInt(playable.length)];
}

class Catalog {
  final http.Client client;
  Catalog({http.Client? client}) : client = client ?? http.Client();

  Future<List<Station>> fetchStations({int limit = 200}) async {
    final uri = Uri.parse(
      'https://all.api.radio-browser.info/json/stations/search'
      '?limit=$limit&hidebroken=true&order=clickcount&reverse=true',
    );
    final resp = await client.get(uri, headers: {
      'User-Agent': 'world-radio-android/1.0',
    });
    if (resp.statusCode != 200 || resp.body.isEmpty) return [];
    final list = jsonDecode(resp.body) as List<dynamic>;
    return list
        .map((e) => Station.fromJson(e as Map<String, dynamic>))
        .where((s) => s.url.isNotEmpty)
        .toList();
  }
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd mobile_flutter && flutter test test/shuffle_test.dart`
Expected: PASS — 3 tests.

- [ ] **Step 6: Commit**

```bash
git add mobile_flutter/lib/domain/station.dart mobile_flutter/lib/data/catalog.dart mobile_flutter/test/shuffle_test.dart
git commit -m "feat: flutter catalog client and shuffle pick"
```

---

### Task 3: Background audio handler (audio_service + just_audio, shuffle on next)

**Files:**
- Create: `mobile_flutter/lib/audio/radio_handler.dart`
- Modify: `mobile_flutter/android/app/src/main/AndroidManifest.xml`
- Modify: `mobile_flutter/lib/main.dart`

**Interfaces:**
- Consumes: `Catalog`, `pickRandom`, `Station` (Task 2).
- Produces: `class RadioHandler extends BaseAudioHandler` with `play()`, `pause()`, `stop()`, `skipToNext()` (= shuffle), `skipToPrevious()` (= previous/shuffle). A top-level `Future<RadioHandler> initAudio()` that registers it with `AudioService`. The handler streams via `just_audio` and publishes `PlaybackState` + `MediaItem` so lock-screen/wheel controls work.

- [ ] **Step 1: Add the audio_service manifest entries** — in `mobile_flutter/android/app/src/main/AndroidManifest.xml`, inside `<manifest>` add permissions and inside `<application>` add the service + receiver (as audio_service requires):

```xml
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK" />
```

inside `<application>`:

```xml
        <service
            android:name="com.ryanheise.audioservice.AudioService"
            android:foregroundServiceType="mediaPlayback"
            android:exported="true">
            <intent-filter>
                <action android:name="android.media.browse.MediaBrowserService" />
            </intent-filter>
        </service>
        <receiver
            android:name="com.ryanheise.audioservice.MediaButtonReceiver"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MEDIA_BUTTON" />
            </intent-filter>
        </receiver>
```

- [ ] **Step 2: Create `mobile_flutter/lib/audio/radio_handler.dart`**

```dart
import 'package:audio_service/audio_service.dart';
import 'package:just_audio/just_audio.dart';
import '../data/catalog.dart';
import '../domain/station.dart';

class RadioHandler extends BaseAudioHandler {
  final _player = AudioPlayer();
  final _catalog = Catalog();
  List<Station> _stations = [];

  RadioHandler() {
    _player.playbackEventStream.listen((_) => _broadcastState());
    _loadStations();
  }

  Future<void> _loadStations() async {
    _stations = await _catalog.fetchStations();
  }

  void _broadcastState() {
    final playing = _player.playing;
    playbackState.add(playbackState.value.copyWith(
      controls: [
        MediaControl.skipToPrevious,
        if (playing) MediaControl.pause else MediaControl.play,
        MediaControl.stop,
        MediaControl.skipToNext,
      ],
      systemActions: const {MediaAction.seek},
      processingState: switch (_player.processingState) {
        ProcessingState.idle => AudioProcessingState.idle,
        ProcessingState.loading => AudioProcessingState.loading,
        ProcessingState.buffering => AudioProcessingState.buffering,
        ProcessingState.ready => AudioProcessingState.ready,
        ProcessingState.completed => AudioProcessingState.completed,
      },
      playing: playing,
    ));
  }

  Future<void> _playStation(Station s) async {
    mediaItem.add(MediaItem(id: s.url, title: s.name, album: 'World Radio'));
    await _player.setUrl(s.url);
    await _player.play();
  }

  Future<void> shuffle() async {
    if (_stations.isEmpty) {
      _stations = await _catalog.fetchStations();
    }
    final pick = pickRandom(_stations);
    if (pick == null) return;
    await _playStation(pick);
  }

  @override
  Future<void> play() => _player.play();

  @override
  Future<void> pause() => _player.pause();

  @override
  Future<void> stop() async {
    await _player.stop();
    await super.stop();
  }

  @override
  Future<void> skipToNext() => shuffle();

  @override
  Future<void> skipToPrevious() => shuffle();
}

Future<RadioHandler> initAudio() {
  return AudioService.init(
    builder: () => RadioHandler(),
    config: const AudioServiceConfig(
      androidNotificationChannelId: 'net.r4dio.radio.channel',
      androidNotificationChannelName: 'World Radio',
      androidNotificationOngoing: true,
    ),
  );
}
```

- [ ] **Step 3: Init the handler in `main.dart`** — change `main()` to async-init audio and pass the handler down:

```dart
import 'package:flutter/material.dart';
import 'audio/radio_handler.dart';

late final RadioHandler audioHandler;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  audioHandler = await initAudio();
  runApp(const WorldRadioApp());
}
```

(Keep the existing `WorldRadioApp` widget from Task 1 for now; Task 4 fills the UI.)

- [ ] **Step 4: Analyze + build**

Run: `cd mobile_flutter && flutter analyze && flutter build apk --debug`
Expected: analyze clean; build succeeds. If audio_service's minSdk is higher than the scaffold default, bump `minSdk` in `android/app/build.gradle.kts` to 23 (audio_service requirement) and rebuild. Report any plugin/Gradle error with its exact text.

- [ ] **Step 5: Commit**

```bash
git add mobile_flutter/lib/audio/radio_handler.dart mobile_flutter/lib/main.dart mobile_flutter/android/app/src/main/AndroidManifest.xml mobile_flutter/android/app/build.gradle.kts
git commit -m "feat: background radio audio handler with shuffle on next"
```

---

### Task 4: Now-playing UI wired to the handler

**Files:**
- Create: `mobile_flutter/lib/ui/home_page.dart`
- Modify: `mobile_flutter/lib/main.dart`

**Interfaces:**
- Consumes: the global `audioHandler` (Task 3) — its `playbackState` and `mediaItem` streams, and `shuffle()` / `play()` / `pause()`.
- Produces: an amber-styled home screen showing the current station, a ⇄ SHUFFLE button, and a ▶/⏸ button reflecting playback state.

- [ ] **Step 1: Create `mobile_flutter/lib/ui/home_page.dart`**

```dart
import 'package:audio_service/audio_service.dart';
import 'package:flutter/material.dart';
import '../main.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    const amber = Color(0xFFFFC457);
    const bright = Color(0xFFFFF0C0);
    return Scaffold(
      backgroundColor: const Color(0xFF15100B),
      body: SafeArea(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Text('▌ r4dio', style: TextStyle(color: amber, fontSize: 18)),
              const SizedBox(height: 16),
              StreamBuilder<MediaItem?>(
                stream: audioHandler.mediaItem,
                builder: (context, snap) => Text(
                  snap.data?.title ?? 'Nothing playing',
                  style: const TextStyle(color: bright, fontSize: 16),
                ),
              ),
              const SizedBox(height: 28),
              StreamBuilder<PlaybackState>(
                stream: audioHandler.playbackState,
                builder: (context, snap) {
                  final playing = snap.data?.playing ?? false;
                  return Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      ElevatedButton(
                        onPressed: () => audioHandler.skipToNext(),
                        child: const Text('⇄ SHUFFLE'),
                      ),
                      const SizedBox(width: 12),
                      ElevatedButton(
                        onPressed: () =>
                            playing ? audioHandler.pause() : audioHandler.play(),
                        child: Text(playing ? '⏸' : '▶'),
                      ),
                    ],
                  );
                },
              ),
            ],
          ),
        ),
      ),
    );
  }
}
```

- [ ] **Step 2: Use `HomePage` in `main.dart`** — set `home: const HomePage()` in `WorldRadioApp` and import `ui/home_page.dart`. Final `main.dart`:

```dart
import 'package:flutter/material.dart';
import 'audio/radio_handler.dart';
import 'ui/home_page.dart';

late final RadioHandler audioHandler;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  audioHandler = await initAudio();
  runApp(const WorldRadioApp());
}

class WorldRadioApp extends StatelessWidget {
  const WorldRadioApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'World Radio',
      theme: ThemeData(scaffoldBackgroundColor: const Color(0xFF15100B)),
      home: const HomePage(),
    );
  }
}
```

- [ ] **Step 3: Analyze + build**

Run: `cd mobile_flutter && flutter analyze && flutter build apk --debug`
Expected: analyze clean; build succeeds.

- [ ] **Step 4: MANUAL smoke test (emulator + phone)** — human runs:

`make mobile-flutter-app` (boots emulator, runs the app). Expected: app opens; SHUFFLE starts a station within a few seconds; ▶/⏸ toggles; a media notification appears; on a real phone (next task's distributed build) locking the screen keeps audio playing and the notification + Bluetooth/steering-wheel play/pause/next control playback.

- [ ] **Step 5: Commit**

```bash
git add mobile_flutter/lib/ui/home_page.dart mobile_flutter/lib/main.dart
git commit -m "feat: now-playing ui wired to audio handler"
```

---

### Task 5: Firebase App Distribution (to the phone)

**Files:**
- Modify: `Makefile` (add distribute target)
- Create: `mobile_flutter/README.md`

**Interfaces:**
- Consumes: the release APK from `flutter build apk --release`.
- Produces: a `make mobile-flutter-distribute` target that uploads the APK to Firebase App Distribution, mirroring the preferans `apk-distribute` pattern.

- [ ] **Step 1: Create the Firebase Android app**

The Firebase CLI is authenticated. Reuse an existing project or create one for radio. To create:
```bash
firebase projects:create r4dio-radio --display-name "World Radio"
firebase apps:create ANDROID "World Radio" --package-name net.r4dio.radio --project r4dio-radio
firebase apps:sdkconfig ANDROID --project r4dio-radio > mobile_flutter/android/app/google-services.json
```
Record the printed **App ID** (format `1:NNNN:android:XXXX`) — the distribute target needs it. If `projects:create` is denied, report the exact error; the user can create the project in the console and give you the project id + app id. `google-services.json` must be gitignored — confirm `mobile_flutter/.gitignore` (from `flutter create`) already ignores it; if not, add it.

- [ ] **Step 2: Add the distribute target to `Makefile`** (fill in the real App ID from Step 1)

```makefile
RADIO_APP_ID ?= <APP_ID_FROM_STEP_1>

.PHONY: mobile-flutter-distribute
mobile-flutter-distribute:
	cd mobile_flutter && flutter build apk --release
	printf "World Radio test build\n\nRecent changes:\n%s\n" "$$(git log -5 --pretty=format:'- %s')" > mobile_flutter/release-notes.txt
	firebase appdistribution:distribute \
	  mobile_flutter/build/app/outputs/flutter-apk/app-release.apk \
	  --app $(RADIO_APP_ID) \
	  --groups internal \
	  --release-notes-file mobile_flutter/release-notes.txt
```

- [ ] **Step 3: Create the tester group + add the user**

```bash
firebase appdistribution:group:create internal "Internal" --project r4dio-radio
firebase appdistribution:testers:add <user-email> --project r4dio-radio
```
Use the user's email. Report if these need console setup.

- [ ] **Step 4: Build + distribute**

Run: `make mobile-flutter-distribute`
Expected: a release APK builds; the CLI prints an uploaded release URL; the tester gets an install link. Report the release URL. (If release signing is required and not configured, `flutter build apk --release` uses a debug-signed APK by default for App Distribution, which is fine for testers; note it.)

- [ ] **Step 5: Create `mobile_flutter/README.md`**

```markdown
# World Radio — Android (Flutter)

Flutter radio player. Background playback (just_audio + audio_service) with lock-screen /
steering-wheel controls; stations from radio-browser.

## Run on emulator

    make mobile-flutter-app

## Distribute (Firebase App Distribution)

    make mobile-flutter-distribute

Sends the release APK to the `internal` tester group. `google-services.json` is not committed.
```

- [ ] **Step 6: Commit** (no `google-services.json`, no `release-notes.txt`)

```bash
git add Makefile mobile_flutter/README.md
git commit -m "chore: firebase app distribution for flutter android"
```

---

## Self-Review Notes

- **Spec coverage:** Flutter scaffold + Makefile reusing preferans pattern (Task 1) · Dart catalog client + shuffle, unit-tested mirroring desktop `pick_random` (Task 2) · audio_service + just_audio background handler with skipToNext=shuffle, foreground media service for lock-screen/Bluetooth/wheel (Task 3) · minimal amber now-playing UI (Task 4) · Firebase App Distribution to the phone (Task 5). Battery: foreground media service, no polling, no visualizer — satisfied by audio_service + UI destroyed when backgrounded. Catalog source is the exact radio-browser search endpoint the desktop uses.
- **Out of scope (per spec):** favorites/sync, search UI, offline cache, Crashlytics/Analytics, Google Play, iOS, Rust reuse, live spectrum — none planned.
- **Reuses preferans infrastructure (per the user's correction):** Makefile targets (`mobile-flutter-emu/app/build-*`), emulator (`Pixel_7`/`emulator-5554`), Firebase App Distribution via app-id + `--groups`. We do NOT pin Java/JDK versions — Flutter's Gradle resolves the toolchain, and Task 1/3 fall back to copying preferans's working `android/` Gradle config if a toolchain error appears.
- **Type consistency:** `Station{uuid,name,url,country,codec,bitrate}` (Task 2) consumed by `pickRandom` (Task 2) and `RadioHandler` (Task 3); `pickRandom(List<Station>, [Random?]): Station?` consistent; `RadioHandler` `shuffle()`/`skipToNext()`/`play()`/`pause()` (Task 3) called by the UI (Task 4); global `audioHandler` (Task 3 main) consumed by HomePage streams (Task 4).
- **Known risks (called out in tasks):** (1) Android toolchain on first Flutter build — fall back to copying preferans's proven `android/` Gradle config (Task 1 Step 6, Task 3 Step 4), not version-pinning; (2) audio_service minSdk → bump to 23 if needed (Task 3 Step 4); (3) `firebase projects:create` permission → console fallback + report (Task 5 Step 1); (4) release signing → debug-signed APK is acceptable for App Distribution (Task 5 Step 4). Each names the exact failure to report.
- **Manual gates:** on-device/emulator audio + wheel controls (Task 4 Step 4) and real distribution (Task 5 Step 4) are human-run.
```
