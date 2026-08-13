package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * a stand-in for a tab whose real content lands in a later task. a centred
 * mono label is all catalog/library need until then; settings adds its own
 * sync pill below this on top of the same bg.
 */
@Composable
fun Placeholder(title: String, subtitle: String, modifier: Modifier = Modifier) {
    val c = R4dioTokens.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg))
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = title,
            color = Color(c.dim),
            fontSize = 16.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.14.em,
            textAlign = TextAlign.Center,
        )
        Text(
            text = subtitle,
            color = Color(c.dim),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}
