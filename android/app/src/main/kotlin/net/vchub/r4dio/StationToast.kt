package net.vchub.r4dio

import android.content.Context
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.TextView

/** how long the panel stays up. long enough to read at a glance, short enough
 *  that it is gone before it can matter to someone driving. */
const val TOAST_MILLIS = 2500L

/** the line the overlay shows. the country earns its place — it is the thing the
 *  filter is set by — but a station with none must not read "name · ". */
fun toastText(name: String, country: String): String = when (country.isBlank()) {
    true -> name
    false -> "$name · $country"
}

/** whether a station change is worth interrupting the screen for.
 *
 *  a pause, a resume, or the same station re-buffering after a network blip all
 *  reach the play path, and firing on those would teach the user to ignore the
 *  panel. only a genuinely different station counts. */
fun shouldAnnounce(previousUuid: String?, nextUuid: String, appIsInForeground: Boolean): Boolean {
    if (appIsInForeground) {
        return false
    }
    return previousUuid != nextUuid
}

/** the overlay draws over other apps, which android grants only from its own
 *  settings screen. it is re-read before every show rather than remembered:
 *  the user can revoke it at any time, and a stale yes would crash the show. */
fun canDrawOverlay(context: Context): Boolean = Settings.canDrawOverlays(context)

/**
 * a label over whatever is on screen, gone on its own. deliberately not a
 * player: anything tappable over a navigation app is a hazard, and anything
 * persistent covers the map the driver is actually looking at.
 */
class StationToast(private val context: Context) {
    private val handler = Handler(Looper.getMainLooper())
    private var view: View? = null

    companion object {
        /** set by MainActivity as it comes and goes. the service and the
         *  activity are separate objects with no lifecycle between them, so the
         *  screen state has to be told, not asked. */
        @Volatile
        var appIsInForeground: Boolean = false
    }

    fun show(text: String) {
        // every failure here is logged: an overlay that does not appear leaves no
        // other trace, and telling "permission missing" apart from "the window
        // manager refused us" is otherwise pure guesswork.
        if (!canDrawOverlay(context)) {
            Log.i("r4dio", "overlay not shown: permission not granted")
            return
        }
        hide()
        val wm = context.getSystemService(Context.WINDOW_SERVICE) as? WindowManager
        if (wm == null) {
            Log.w("r4dio", "overlay not shown: no window manager")
            return
        }
        val panel = TextView(context).apply {
            this.text = text
            setTextColor(context.getColor(R.color.amber_hi))
            setBackgroundResource(R.drawable.bg_toast)
            textSize = 15f
            setPadding(36, 22, 36, 22)
        }
        val type = when (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            true -> WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
            false -> @Suppress("DEPRECATION") WindowManager.LayoutParams.TYPE_PHONE
        }
        val params = WindowManager.LayoutParams(
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            type,
            // not focusable and not touchable: the panel must never take a tap
            // meant for the map underneath it.
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
            y = 120
        }
        runCatching { wm.addView(panel, params) }
            .onSuccess {
                view = panel
                handler.postDelayed({ hide() }, TOAST_MILLIS)
            }
            .onFailure { Log.w("r4dio", "overlay not shown: ${it.message}") }
    }

    fun hide() {
        val panel = view ?: return
        view = null
        handler.removeCallbacksAndMessages(null)
        val wm = context.getSystemService(Context.WINDOW_SERVICE) as? WindowManager ?: return
        // the window may already be gone if the process was torn down under us.
        runCatching { wm.removeView(panel) }
    }
}
