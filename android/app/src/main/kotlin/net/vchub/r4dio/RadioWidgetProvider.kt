package net.vchub.r4dio

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.view.View
import android.widget.RemoteViews
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors

const val ACTION_WIDGET_SHUFFLE = "net.vchub.r4dio.WIDGET_SHUFFLE"
const val ACTION_WIDGET_TOGGLE = "net.vchub.r4dio.WIDGET_TOGGLE"
const val ACTION_WIDGET_STAR = "net.vchub.r4dio.WIDGET_STAR"

class RadioWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(context: Context, mgr: AppWidgetManager, ids: IntArray) {
        val prefs = context.getSharedPreferences("widget", Context.MODE_PRIVATE)
        val station = prefs.getString("station", "") ?: ""
        val meta = prefs.getString("meta", "") ?: ""
        val isPlaying = prefs.getBoolean("is_playing", false)
        val isFav = prefs.getBoolean("is_fav", false)
        ids.forEach { render(context, mgr, it, station, meta, isPlaying, isFav) }
    }

    // without this, resizing a widget keeps rendering the layout chosen at add time.
    override fun onAppWidgetOptionsChanged(
        context: Context,
        mgr: AppWidgetManager,
        id: Int,
        newOptions: android.os.Bundle,
    ) {
        onUpdate(context, mgr, intArrayOf(id))
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val cmd = when (intent.action) {
            ACTION_WIDGET_SHUFFLE -> CMD_SHUFFLE
            ACTION_WIDGET_TOGGLE -> CMD_TOGGLE
            ACTION_WIDGET_STAR -> CMD_STAR
            else -> null
        }
        cmd ?: return
        val pending = goAsync()
        val token = SessionToken(
            context.applicationContext,
            ComponentName(context.applicationContext, PlaybackService::class.java),
        )
        val future = MediaController.Builder(context.applicationContext, token).buildAsync()
        future.addListener({
            val controller = runCatching { future.get() }.getOrNull()
            if (controller == null) {
                pending.finish()
                return@addListener
            }
            val handler = Handler(Looper.getMainLooper())
            var released = false
            var listener: Player.Listener? = null
            val releaseOnce = {
                if (!released) {
                    released = true
                    handler.removeCallbacksAndMessages(null)
                    listener?.let { controller.removeListener(it) }
                    controller.release()
                    pending.finish()
                }
            }
            controller.sendCustomCommand(
                SessionCommand(cmd, android.os.Bundle.EMPTY),
                android.os.Bundle.EMPTY,
            )
            val l = object : Player.Listener {
                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    if (isPlaying) {
                        releaseOnce()
                    }
                }
            }
            listener = l
            controller.addListener(l)
            if (controller.isPlaying) {
                releaseOnce()
            }
            handler.postDelayed({ releaseOnce() }, 15000)
        }, MoreExecutors.directExecutor())
    }

    companion object {
        fun refresh(context: Context, station: String, meta: String, isPlaying: Boolean, isFav: Boolean) {
            context.getSharedPreferences("widget", Context.MODE_PRIVATE).edit()
                .putString("station", station)
                .putString("meta", meta)
                .putBoolean("is_playing", isPlaying)
                .putBoolean("is_fav", isFav)
                .apply()
            val mgr = AppWidgetManager.getInstance(context)
            val ids = mgr.getAppWidgetIds(ComponentName(context, RadioWidgetProvider::class.java))
            ids.forEach { render(context, mgr, it, station, meta, isPlaying, isFav) }
        }

        private fun render(
            context: Context,
            mgr: AppWidgetManager,
            id: Int,
            station: String,
            meta: String,
            isPlaying: Boolean,
            isFav: Boolean,
        ) {
            // the launcher reports the current cell size per widget instance, so a user
            // with both a wide and a narrow copy gets the right layout for each.
            val widthDp = mgr.getAppWidgetOptions(id)
                .getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH, 0)
            val layout = when (usesCompactLayout(widthDp)) {
                true -> R.layout.widget_radio_small
                false -> R.layout.widget_radio
            }
            val views = RemoteViews(context.packageName, layout)
            views.setTextViewText(
                R.id.widget_station,
                widgetStationLabel(station, context.getString(R.string.widget_idle)),
            )
            views.setTextViewText(R.id.widget_meta, meta)
            views.setViewVisibility(R.id.widget_meta, if (meta.isBlank()) View.GONE else View.VISIBLE)
            views.setViewVisibility(R.id.widget_live, if (isPlaying) View.VISIBLE else View.GONE)
            views.setImageViewResource(
                R.id.widget_toggle,
                if (isPlaying) R.drawable.ic_pause else R.drawable.ic_play,
            )
            views.setImageViewResource(
                R.id.widget_star,
                if (isFav) R.drawable.ic_star else R.drawable.ic_star_outline,
            )
            views.setOnClickPendingIntent(R.id.widget_shuffle, broadcastPending(context, ACTION_WIDGET_SHUFFLE))
            views.setOnClickPendingIntent(R.id.widget_toggle, broadcastPending(context, ACTION_WIDGET_TOGGLE))
            views.setOnClickPendingIntent(R.id.widget_star, broadcastPending(context, ACTION_WIDGET_STAR))
            views.setOnClickPendingIntent(R.id.widget_root, openAppPending(context))
            mgr.updateAppWidget(id, views)
        }

        private fun openAppPending(context: Context): PendingIntent {
            val intent = Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            return PendingIntent.getActivity(
                context, 0, intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }

        private fun broadcastPending(context: Context, action: String): PendingIntent {
            val intent = Intent(context, RadioWidgetProvider::class.java).setAction(action)
            return PendingIntent.getBroadcast(
                context, action.hashCode(), intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
    }
}
