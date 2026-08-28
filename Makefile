ANDROID_HOME ?= $(HOME)/android
JAVA_HOME    ?= /Applications/Android Studio.app/Contents/jbr/Contents/Home
export JAVA_HOME
EMULATOR_ID ?= $(shell $(ANDROID_HOME)/emulator/emulator -list-avds 2>/dev/null | head -1)
ADB := $(ANDROID_HOME)/platform-tools/adb

.PHONY: check-window
# the macos window is javascript, which `cargo test` never looks at: a duplicate
# const once shipped a blank window with every rust test green. this parses every
# module and drives the now-playing panel against a fake backend.
check-window:
	node --experimental-vm-modules crates/r4dio-macos/ui/check.mjs

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

RADIO_APP_ID ?= 1:106521889249:android:aa2fa06ebc14c4cd532aa0
# set this in your shell or a local env file — this repository is public, so a
# personal address does not belong in it:  export RADIO_TESTERS=you@example.com
RADIO_TESTERS ?=

.PHONY: android-distribute
# the check runs before android-build, not after: failing only once the apk is
# already built wastes a full gradle run to tell you a variable is unset.
android-distribute:
	@test -n "$(RADIO_TESTERS)" || { echo "RADIO_TESTERS is empty — set it, or the build goes to nobody" >&2; exit 1; }
	$(MAKE) android-build
	printf "World Radio test build\n\nRecent changes:\n%s\n" "$$(git log -5 --pretty=format:'- %s')" > android/release-notes.txt
	firebase appdistribution:distribute \
	  android/app/build/outputs/apk/debug/app-debug.apk \
	  --app $(RADIO_APP_ID) \
	  --testers "$(RADIO_TESTERS)" \
	  --release-notes-file android/release-notes.txt \
	  --project r4dio-vchub
