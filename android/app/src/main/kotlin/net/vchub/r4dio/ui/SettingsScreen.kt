package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.R

/**
 * themes, hidden countries and the door to sync.
 *
 * the theme is a synced value the desktop also writes, so picking one here
 * changes every linked device — which is why each row previews its own palette
 * rather than only naming it.
 */
@Composable
fun SettingsScreen(
    theme: String,
    hiddenCountries: Set<String>,
    fillOnMobile: Boolean,
    onTheme: (String) -> Unit,
    onShowCountry: (String) -> Unit,
    onFillOnMobile: (() -> Unit)?,
    onOpenSync: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    LazyColumn(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg))
            .padding(horizontal = 16.dp),
    ) {
        item {
            GroupTitle(R.string.settings_theme)
        }
        items(THEME_SLUGS, key = { it }) { slug ->
            ThemeRow(slug = slug, selected = slug == theme, onPick = { onTheme(slug) })
        }
        item {
            GroupTitle(R.string.settings_hidden)
            when {
                hiddenCountries.isEmpty() -> Hint(stringResource(R.string.settings_hidden_none))
                else -> HiddenRow(hiddenCountries, onShowCountry)
            }
        }
        item {
            GroupTitle(R.string.settings_data)
            if (onFillOnMobile != null) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Pill(
                        text = stringResource(R.string.settings_fill_on_mobile),
                        on = fillOnMobile,
                        onClick = onFillOnMobile,
                    )
                }
                Hint(stringResource(R.string.settings_fill_on_mobile_hint))
            }
        }
        item {
            GroupTitle(R.string.settings_account)
            Row(
                modifier = Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Pill(text = stringResource(R.string.settings_open_sync), on = true, onClick = onOpenSync)
            }
        }
    }
}

/**
 * a row that shows the theme rather than describing it: three dots in the
 * palette's own background, foreground and accent. a name alone would make
 * choosing between fourteen a matter of guessing.
 */
@Composable
private fun ThemeRow(slug: String, selected: Boolean, onPick: () -> Unit) {
    val c = R4dioTokens.colors
    val palette = paletteFor(slug) ?: return
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = 6.dp)
            .background(Color(c.panel()), shape)
            .border(1.dp, Color(if (selected) c.accent else c.rule()), shape)
            .clickable(onClick = onPick)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            listOf(palette.bg, palette.fg, palette.accent).forEach { swatch ->
                Box(
                    modifier = Modifier
                        .size(14.dp)
                        .background(Color(swatch), CircleShape)
                        .border(1.dp, Color(c.rule()), CircleShape),
                )
            }
        }
        Text(
            text = slug.uppercase(),
            color = Color(if (selected) c.accent else c.fg),
            fontSize = 12.sp,
            fontFamily = MonoFamily,
            letterSpacing = 0.1.em,
            modifier = Modifier.padding(start = 12.dp),
        )
        Spacer(modifier = Modifier.weight(1f))
        if (selected) {
            Text(
                text = "✓",
                color = Color(c.accent),
                fontSize = 13.sp,
                fontFamily = MonoFamily,
            )
        }
    }
}

/** each hidden country as a chip that un-hides it — the only way back. */
@Composable
private fun HiddenRow(countries: Set<String>, onShow: (String) -> Unit) {
    Column(modifier = Modifier.fillMaxWidth().padding(top = 4.dp)) {
        countries.sorted().chunked(4).forEach { row ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                row.forEach { code ->
                    Pill(text = "$code ✕", on = true, onClick = { onShow(code) })
                }
            }
        }
    }
}

@Composable
private fun GroupTitle(res: Int) {
    val c = R4dioTokens.colors
    Text(
        text = stringResource(res),
        color = Color(c.dim),
        fontSize = 10.sp,
        fontWeight = FontWeight.Bold,
        fontFamily = MonoFamily,
        letterSpacing = 0.18.em,
        modifier = Modifier.padding(top = 20.dp, bottom = 8.dp),
    )
}

@Composable
private fun Hint(text: String) {
    val c = R4dioTokens.colors
    Text(
        text = text,
        color = Color(c.mute()),
        fontSize = 11.sp,
        fontFamily = MonoFamily,
        modifier = Modifier.padding(top = 6.dp),
    )
}
