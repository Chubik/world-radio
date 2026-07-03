package net.vchub.r4dio

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews

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
            views.setOnClickPendingIntent(R.id.widget_shuffle, servicePending(context, ACTION_WIDGET_SHUFFLE))
            views.setOnClickPendingIntent(R.id.widget_toggle, servicePending(context, ACTION_WIDGET_TOGGLE))
            mgr.updateAppWidget(id, views)
        }

        private fun servicePending(context: Context, action: String): PendingIntent {
            val intent = Intent(context, PlaybackService::class.java).setAction(action)
            return PendingIntent.getService(
                context, action.hashCode(), intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
    }
}
