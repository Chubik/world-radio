package net.vchub.r4dio

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.widget.RemoteViews
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors

const val ACTION_WIDGET_SHUFFLE = "net.vchub.r4dio.WIDGET_SHUFFLE"
const val ACTION_WIDGET_TOGGLE = "net.vchub.r4dio.WIDGET_TOGGLE"
const val EXTRA_WIDGET_STATION = "net.vchub.r4dio.WIDGET_STATION"

class RadioWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(context: Context, mgr: AppWidgetManager, ids: IntArray) {
        val prefs = context.getSharedPreferences("widget", Context.MODE_PRIVATE)
        val station = prefs.getString("station", "r4dio") ?: "r4dio"
        val isPlaying = prefs.getBoolean("is_playing", false)
        ids.forEach { render(context, mgr, it, station, isPlaying) }
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val cmd = when (intent.action) {
            ACTION_WIDGET_SHUFFLE -> CMD_SHUFFLE
            ACTION_WIDGET_TOGGLE -> CMD_TOGGLE
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
        fun refresh(context: Context, station: String, isPlaying: Boolean) {
            context.getSharedPreferences("widget", Context.MODE_PRIVATE)
                .edit().putString("station", station).putBoolean("is_playing", isPlaying).apply()
            val mgr = AppWidgetManager.getInstance(context)
            val ids = mgr.getAppWidgetIds(ComponentName(context, RadioWidgetProvider::class.java))
            ids.forEach { render(context, mgr, it, station, isPlaying) }
        }

        private fun render(context: Context, mgr: AppWidgetManager, id: Int, station: String, isPlaying: Boolean) {
            val views = RemoteViews(context.packageName, R.layout.widget_radio)
            views.setTextViewText(R.id.widget_station, station)
            views.setImageViewResource(R.id.widget_toggle, if (isPlaying) R.drawable.ic_pause else R.drawable.ic_play)
            views.setOnClickPendingIntent(R.id.widget_shuffle, broadcastPending(context, ACTION_WIDGET_SHUFFLE))
            views.setOnClickPendingIntent(R.id.widget_toggle, broadcastPending(context, ACTION_WIDGET_TOGGLE))
            mgr.updateAppWidget(id, views)
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
