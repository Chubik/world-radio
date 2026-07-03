package net.vchub.r4dio

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import java.io.ByteArrayOutputStream

fun crtArtworkPng(size: Int = 512): ByteArray {
    val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bmp)
    val bg = Paint().apply { color = Color.parseColor("#1b1510"); isAntiAlias = true }
    canvas.drawRect(0f, 0f, size.toFloat(), size.toFloat(), bg)
    val bar = Paint().apply { color = Color.parseColor("#ffc457"); isAntiAlias = true }
    val w = size * 0.14f
    val h = size * 0.42f
    val left = (size - w) / 2f
    val top = (size - h) / 2f
    canvas.drawRoundRect(RectF(left, top, left + w, top + h), w * 0.3f, w * 0.3f, bar)
    val out = ByteArrayOutputStream()
    bmp.compress(Bitmap.CompressFormat.PNG, 100, out)
    bmp.recycle()
    return out.toByteArray()
}
