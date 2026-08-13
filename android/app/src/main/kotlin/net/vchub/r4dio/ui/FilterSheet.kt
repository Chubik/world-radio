package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.BITRATE_STEPS
import net.vchub.r4dio.CatalogFilters
import net.vchub.r4dio.R
import net.vchub.r4dio.toggleValue

/** the rows one group of the sheet offers: a label, the count behind it, and the
 *  value that goes into the filter set in the case searchCatalog expects. */
data class FacetRow(val label: String, val value: String, val count: Int)

/** everything the sheet lists, computed once by the screen so toggling a row
 *  never recomputes a facet pass over 58k stations. */
data class FacetSets(
    val countries: List<FacetRow> = emptyList(),
    val genres: List<FacetRow> = emptyList(),
    val codecs: List<FacetRow> = emptyList(),
)

/**
 * the filter modal. edits a pending copy rather than the live filters, so the
 * list behind it does not thrash while the user is still choosing; the bottom
 * button is what commits.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FilterSheet(
    facets: FacetSets,
    applied: CatalogFilters,
    countFor: (CatalogFilters) -> Int,
    onApply: (CatalogFilters) -> Unit,
    onDismiss: () -> Unit,
) {
    val c = R4dioTokens.colors
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var pending by remember { mutableStateOf(applied) }
    val matches = countFor(pending)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = Color(c.bg),
        contentColor = Color(c.fg),
        dragHandle = null,
        // the sheet is its own window and does not inherit the activity's inset,
        // so at full height its header would sit under the status bar.
        contentWindowInsets = { WindowInsets.systemBars },
    ) {
        Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
            SheetHeader(onReset = { pending = CatalogFilters() }, onDismiss = onDismiss)
            LazyColumn(modifier = Modifier.weight(1f, fill = false)) {
                group(
                    title = R.string.filters_country,
                    rows = facets.countries,
                    selected = pending.countries,
                ) { pending = pending.copy(countries = toggleValue(pending.countries, it)) }
                group(
                    title = R.string.filters_genre,
                    rows = facets.genres,
                    selected = pending.genres,
                ) { pending = pending.copy(genres = toggleValue(pending.genres, it)) }
                group(
                    title = R.string.filters_codec,
                    rows = facets.codecs,
                    selected = pending.codecs,
                ) { pending = pending.copy(codecs = toggleValue(pending.codecs, it)) }
                item {
                    GroupTitle(R.string.filters_bitrate)
                    BitrateRow(pending.minBitrate) { pending = pending.copy(minBitrate = it) }
                }
            }
            ShowButton(matches) {
                onApply(pending)
                onDismiss()
            }
        }
    }
}

@Composable
private fun SheetHeader(onReset: () -> Unit, onDismiss: () -> Unit) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 18.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.filters_title),
            color = Color(c.accent),
            fontSize = 14.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.16.em,
        )
        Spacer(modifier = Modifier.weight(1f))
        Pill(text = stringResource(R.string.filters_reset), on = false, onClick = onReset)
        Text(
            text = stringResource(R.string.sync_done),
            color = Color(c.dim),
            fontSize = 15.sp,
            fontFamily = MonoFamily,
            modifier = Modifier.clickable { onDismiss() }.padding(start = 14.dp, end = 4.dp),
        )
    }
}

/** a titled block of toggle rows. skipped entirely when the catalogue offers
 *  nothing for it, so the sheet never shows an empty heading. */
private fun androidx.compose.foundation.lazy.LazyListScope.group(
    title: Int,
    rows: List<FacetRow>,
    selected: Set<String>,
    onToggle: (String) -> Unit,
) {
    if (rows.isEmpty()) return
    item { GroupTitle(title) }
    items(rows, key = { "${title}_${it.value}" }) { row ->
        FacetToggle(row, row.value in selected) { onToggle(row.value) }
    }
}

@Composable
private fun GroupTitle(title: Int) {
    val c = R4dioTokens.colors
    Text(
        text = stringResource(title),
        color = Color(c.dim),
        fontSize = 10.sp,
        fontWeight = FontWeight.Bold,
        fontFamily = MonoFamily,
        letterSpacing = 0.2.em,
        modifier = Modifier.padding(top = 16.dp, bottom = 6.dp),
    )
}

@Composable
private fun FacetToggle(row: FacetRow, on: Boolean, onToggle: () -> Unit) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(42.dp)
            .clickable { onToggle() },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = if (on) "▣" else "▢",
            color = Color(if (on) c.accent else c.dim),
            fontSize = 14.sp,
            fontFamily = MonoFamily,
        )
        Text(
            text = row.label,
            color = Color(if (on) c.accent else c.fg),
            fontSize = 13.sp,
            fontFamily = MonoFamily,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).padding(start = 12.dp),
        )
        Text(
            text = row.count.toString(),
            color = Color(c.mute()),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
        )
    }
}

/** exclusive rather than a set: a minimum is one number, and offering steps
 *  keeps it thumb-sized where a slider in a car would not be. */
@Composable
private fun BitrateRow(current: Int, onPick: (Int) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        BITRATE_STEPS.forEach { step ->
            Pill(
                text = when (step) {
                    0 -> stringResource(R.string.filters_any)
                    else -> stringResource(R.string.catalog_bitrate_k, step)
                },
                on = step == current,
                onClick = { onPick(step) },
            )
        }
    }
}

@Composable
private fun ShowButton(matches: Int, onApply: () -> Unit) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(14.dp)
    // nothing to show is not a button worth pressing, so it reads as the dead
    // end it is rather than applying a filter that empties the screen.
    val live = matches > 0
    val tone = Color(if (live) c.accent else c.dim)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 12.dp, bottom = 20.dp)
            .height(58.dp)
            .background(Color(c.panel()), shape)
            .border(1.dp, tone, shape)
            .let { if (live) it.clickable { onApply() } else it },
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = when (live) {
                true -> stringResource(R.string.filters_show_n, matches)
                false -> stringResource(R.string.filters_show_none)
            },
            color = tone,
            fontSize = 14.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.14.em,
        )
    }
}
