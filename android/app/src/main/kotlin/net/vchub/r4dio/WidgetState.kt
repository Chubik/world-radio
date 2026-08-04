package net.vchub.r4dio

/**
 * widget decisions kept free of android types so they can be unit tested —
 * RemoteViews and AppWidgetManager have no test harness in this project.
 */

/** widths at or below this get the stacked 2x1 layout; above it, the 4x1 row. */
const val WIDGET_SMALL_MAX_WIDTH_DP = 250

/**
 * a launcher that has not measured the widget yet reports 0. treat that as wide:
 * the full layout degrades legibly when squeezed, the compact one wastes a wide cell.
 */
fun usesCompactLayout(widthDp: Int): Boolean = widthDp in 1..WIDGET_SMALL_MAX_WIDTH_DP

fun widgetStationLabel(station: String, idle: String): String =
    station.ifBlank { idle }

fun widgetMetaLabel(country: String, codec: String, bitrate: Int): String =
    listOf(country, codec, if (bitrate > 0) "${bitrate}k" else "")
        .filter { it.isNotBlank() }
        .joinToString(" · ")
