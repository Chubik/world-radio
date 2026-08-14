package net.vchub.r4dio.ui

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.OFFERED_COUNTRY_CODES
import net.vchub.r4dio.R
import net.vchub.r4dio.isSyncKey

/**
 * the account screen: link this phone to a desktop, or unlink it.
 *
 * two states rather than one screen of half-active controls — without a key
 * there is nothing to show or copy, and with one there is nothing to create.
 */
@Composable
fun SyncScreen(
    key: String?,
    qr: Bitmap?,
    hiddenCountries: Set<String>,
    busy: Boolean,
    onUseKey: (String) -> Unit,
    onCreate: () -> Unit,
    onScan: () -> Unit,
    onCopy: () -> Unit,
    onLogOut: () -> Unit,
    onDelete: () -> Unit,
    onExport: () -> Unit,
    onImport: () -> Unit,
    onHiddenCountries: (Set<String>) -> Unit,
    onFeedback: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg))
            .windowInsetsPadding(WindowInsets.systemBars)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 14.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.sync_title),
                color = Color(c.accent),
                fontSize = 15.sp,
                fontWeight = FontWeight.Bold,
                fontFamily = MonoFamily,
                letterSpacing = 0.14.em,
            )
            Spacer(modifier = Modifier.weight(1f))
            Pill(text = stringResource(R.string.now_close), on = false, onClick = onClose)
        }
        Text(
            text = stringResource(R.string.sync_lede),
            color = Color(c.mute()),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
            modifier = Modifier.padding(bottom = 6.dp),
        )

        when (key) {
            null -> Unlinked(busy = busy, onUseKey = onUseKey, onCreate = onCreate, onScan = onScan)
            else -> Linked(key = key, qr = qr, onCopy = onCopy, onLogOut = onLogOut, onDelete = onDelete)
        }

        SectionTitle(R.string.sync_hide_countries)
        CountryGrid(hidden = hiddenCountries, onToggle = { code ->
            val next = when (code in hiddenCountries) {
                true -> hiddenCountries - code
                false -> hiddenCountries + code
            }
            onHiddenCountries(next)
        })

        SectionTitle(R.string.sync_backup_label)
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Pill(text = stringResource(R.string.sync_export), on = false, onClick = onExport)
            Pill(text = stringResource(R.string.sync_import), on = false, onClick = onImport)
        }
        Text(
            text = stringResource(R.string.sync_backup_hint),
            color = Color(c.mute()),
            fontSize = 10.sp,
            fontFamily = MonoFamily,
            modifier = Modifier.padding(top = 8.dp),
        )

        Text(
            text = stringResource(R.string.sync_feedback),
            color = Color(c.dim),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
            modifier = Modifier
                .padding(top = 26.dp, bottom = 28.dp)
                .clickable(onClick = onFeedback),
        )
    }
}

@Composable
private fun Unlinked(busy: Boolean, onUseKey: (String) -> Unit, onCreate: () -> Unit, onScan: () -> Unit) {
    val c = R4dioTokens.colors
    var typed by remember { mutableStateOf("") }
    SectionTitle(R.string.sync_key_label)
    val shape = RoundedCornerShape(12.dp)
    BasicTextField(
        value = typed,
        onValueChange = { typed = it },
        singleLine = true,
        textStyle = TextStyle(
            color = Color(c.fg),
            fontSize = 13.sp,
            fontFamily = MonoFamily,
        ),
        cursorBrush = SolidColor(Color(c.accent)),
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(c.panel()), shape)
            .border(1.dp, Color(c.rule()), shape)
            .padding(horizontal = 14.dp, vertical = 14.dp),
        decorationBox = { inner ->
            when (typed.isEmpty()) {
                true -> Text(
                    text = stringResource(R.string.sync_key_hint),
                    color = Color(c.mute()),
                    fontSize = 13.sp,
                    fontFamily = MonoFamily,
                )
                false -> {}
            }
            inner()
        },
    )
    Row(
        modifier = Modifier.padding(top = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // only offered once what is typed could actually be a key, so the button
        // cannot produce "invalid key" for something obviously not one.
        Pill(
            text = stringResource(R.string.sync_use_key),
            on = isSyncKey(typed),
            onClick = { onUseKey(typed.trim()) },
        )
        Pill(text = stringResource(R.string.sync_scan), on = false, onClick = onScan)
    }
    SectionTitle(R.string.sync_or)
    Pill(
        // each create mints a new account server-side, so a second tap while the
        // first is in flight would strand an orphan.
        text = stringResource(R.string.sync_create),
        on = !busy,
        onClick = { if (!busy) onCreate() },
    )
}

@Composable
private fun Linked(key: String, qr: Bitmap?, onCopy: () -> Unit, onLogOut: () -> Unit, onDelete: () -> Unit) {
    val c = R4dioTokens.colors
    SectionTitle(R.string.sync_your_key)
    Text(
        text = key,
        color = Color(c.fg),
        fontSize = 13.sp,
        fontFamily = MonoFamily,
        modifier = Modifier.padding(bottom = 8.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Pill(text = stringResource(R.string.sync_copy), on = false, onClick = onCopy)
        Pill(text = stringResource(R.string.sync_logout), on = false, onClick = onLogOut)
        Pill(text = stringResource(R.string.sync_delete), on = false, onClick = onDelete)
    }
    if (qr != null) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(top = 18.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Image(
                bitmap = qr.asImageBitmap(),
                contentDescription = stringResource(R.string.sync_qr_caption),
                modifier = Modifier.size(220.dp),
            )
            Text(
                text = stringResource(R.string.sync_qr_caption),
                color = Color(c.mute()),
                fontSize = 10.sp,
                fontFamily = MonoFamily,
                letterSpacing = 0.14.em,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}

/** the curated forty, four to a row so the whole set is visible without scrolling far. */
@Composable
private fun CountryGrid(hidden: Set<String>, onToggle: (String) -> Unit) {
    Column(modifier = Modifier.fillMaxWidth()) {
        OFFERED_COUNTRY_CODES.chunked(5).forEach { row ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                row.forEach { code ->
                    Pill(text = code, on = code in hidden, onClick = { onToggle(code) })
                }
            }
        }
    }
}

@Composable
private fun SectionTitle(res: Int) {
    val c = R4dioTokens.colors
    Text(
        text = stringResource(res),
        color = Color(c.dim),
        fontSize = 10.sp,
        fontWeight = FontWeight.Bold,
        fontFamily = MonoFamily,
        letterSpacing = 0.18.em,
        modifier = Modifier.padding(top = 22.dp, bottom = 8.dp),
    )
}
