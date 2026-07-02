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
