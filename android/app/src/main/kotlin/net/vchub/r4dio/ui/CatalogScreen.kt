package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import net.vchub.r4dio.CatalogFilters
import net.vchub.r4dio.R
import net.vchub.r4dio.Station
import net.vchub.r4dio.activeChips
import net.vchub.r4dio.codecFacets
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import net.vchub.r4dio.filtersToList
import net.vchub.r4dio.filtersFromList
import net.vchub.r4dio.countryFacets
import net.vchub.r4dio.genreFacets
import net.vchub.r4dio.offeredCodecRows
import net.vchub.r4dio.offeredCountryRows
import net.vchub.r4dio.offeredGenreRows
import net.vchub.r4dio.searchCatalog
import net.vchub.r4dio.withoutChip

/**
 * browse the whole cached catalogue. every list on this screen comes from
 * [searchCatalog], which applies the ban before anything else — nothing here
 * ever renders raw cache contents.
 *
 * [loading] rather than an empty list while the 10mb cache is being read: an
 * empty catalogue and an unread one look identical to the user otherwise.
 */
@Composable
fun CatalogScreen(
    stations: List<Station>,
    favourites: Set<String>,
    blocked: Set<String>,
    onPlay: (Station) -> Unit,
    onStar: (Station) -> Unit,
    onBlock: (Station) -> Unit,
    modifier: Modifier = Modifier,
    loading: Boolean = false,
) {
    val c = R4dioTokens.colors
    var query by rememberSaveable { mutableStateOf("") }
    var filters by rememberSaveable(stateSaver = FiltersSaver) { mutableStateOf(CatalogFilters()) }
    var sheetOpen by rememberSaveable { mutableStateOf(false) }

    // one pass over 58k stations per keystroke or filter change, off the ui
    // thread. derivedStateOf alone would run it on the composing thread.
    var results by remember { mutableStateOf<List<Station>>(emptyList()) }
    LaunchedEffect(stations, query, filters) {
        results = withContext(Dispatchers.Default) { searchCatalog(stations, query, filters) }
    }

    val chips by remember(filters) { derivedStateOf { activeChips(filters) } }
    val listState = rememberLazyListState()
    // a narrowed list the user cannot see the top of reads as "nothing changed".
    LaunchedEffect(query, filters) { listState.scrollToItem(0) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg))
            .padding(horizontal = 16.dp),
    ) {
        SearchField(query) { query = it }
        ChipRow(
            chips = chips,
            onOpenFilters = { sheetOpen = true },
            onDrop = { filters = withoutChip(filters, it) },
            onClearAll = { filters = CatalogFilters() },
        )
        Box(modifier = Modifier.weight(1f)) {
            when {
                loading -> Notice(stringResource(R.string.catalog_loading))
                results.isEmpty() -> EmptyState(query, filters) {
                    query = ""
                    filters = CatalogFilters()
                }
                else -> LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
                    items(results, key = { it.uuid }) { station ->
                        StationRow(
                            station = station,
                            starred = station.uuid in favourites,
                            blocked = station.uuid in blocked,
                            onPlay = { onPlay(station) },
                            onStar = { onStar(station) },
                            onBlock = { onBlock(station) },
                        )
                    }
                }
            }
        }
    }

    if (sheetOpen) {
        // the facet passes are the expensive ones, so they are computed once for
        // the catalogue rather than per toggle. the sheet's live count is a plain
        // searchCatalog call, which the measured cost makes free.
        // three passes over 58k stations, and the genre pass splits every tag
        // string — on the composing thread that is a freeze when the sheet opens.
        var facets by remember(stations) { mutableStateOf(FacetSets()) }
        LaunchedEffect(stations) {
            facets = withContext(Dispatchers.Default) {
                FacetSets(
                    countries = offeredCountryRows(
                        countryFacets(stations).filter { it.first.isNotBlank() },
                    ).map { FacetRow(it.first, it.first, it.second) },
                    genres = offeredGenreRows(genreFacets(stations))
                        .map { FacetRow(it.first.uppercase(), it.first, it.second) },
                    codecs = offeredCodecRows(codecFacets(stations))
                        .map { FacetRow(it.first, it.first, it.second) },
                )
            }
        }
        FilterSheet(
            facets = facets,
            applied = filters,
            countFor = { searchCatalog(stations, query, it).size },
            onApply = { filters = it },
            onDismiss = { sheetOpen = false },
        )
    }
}

@Composable
private fun SearchField(query: String, onQuery: (String) -> Unit) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(14.dp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 10.dp)
            .height(50.dp)
            .background(Color(c.panel()), shape)
            .border(1.dp, Color(c.rule()), shape)
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = "⌕",
            color = Color(c.accent),
            fontSize = 16.sp,
            fontFamily = MonoFamily,
        )
        Box(modifier = Modifier.weight(1f).padding(start = 10.dp)) {
            if (query.isEmpty()) {
                Text(
                    text = stringResource(R.string.catalog_search_hint),
                    color = Color(c.dim),
                    fontSize = 14.sp,
                    fontFamily = MonoFamily,
                )
            }
            BasicTextField(
                value = query,
                onValueChange = onQuery,
                singleLine = true,
                textStyle = TextStyle(
                    color = Color(c.fg),
                    fontSize = 14.sp,
                    fontFamily = MonoFamily,
                ),
                cursorBrush = SolidColor(Color(c.accent)),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (query.isNotEmpty()) {
            Text(
                text = "✕",
                color = Color(c.dim),
                fontSize = 14.sp,
                fontFamily = MonoFamily,
                modifier = Modifier.clickable { onQuery("") }.padding(start = 8.dp),
            )
        }
    }
}

/**
 * what is in force, and the two ways out of it. scrolls sideways rather than
 * wrapping: a chip row that grows downwards eats the list it describes.
 */
@Composable
private fun ChipRow(
    chips: List<net.vchub.r4dio.FilterChip>,
    onOpenFilters: () -> Unit,
    onDrop: (net.vchub.r4dio.FilterChip) -> Unit,
    onClearAll: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 10.dp, bottom = 8.dp)
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Pill(text = stringResource(R.string.catalog_filters), on = chips.isNotEmpty(), onClick = onOpenFilters)
        chips.forEach { chip ->
            Pill(text = "${chip.label} ✕", on = true, onClick = { onDrop(chip) })
        }
        if (chips.isNotEmpty()) {
            Pill(text = stringResource(R.string.catalog_clear_all), on = false, onClick = onClearAll)
        }
    }
}

@Composable
private fun EmptyState(query: String, filters: CatalogFilters, onClear: () -> Unit) {
    val c = R4dioTokens.colors
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.catalog_empty_title),
            color = Color(c.dim),
            fontSize = 15.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.14.em,
            textAlign = TextAlign.Center,
        )
        // spelled out rather than "try again": the user cannot fix a filter they
        // cannot see, and the chip row may have scrolled the offender off screen.
        val spelled = emptyStateDetail(query, filters)
        if (spelled.isNotBlank()) {
            Text(
                text = spelled,
                color = Color(c.mute()),
                fontSize = 11.sp,
                fontFamily = MonoFamily,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 10.dp),
            )
        }
        Box(modifier = Modifier.padding(top = 18.dp)) {
            Pill(text = stringResource(R.string.catalog_empty_clear), on = true, onClick = onClear)
        }
    }
}

/** every narrowing in force, in one line, so the empty screen names its cause. */
fun emptyStateDetail(query: String, filters: CatalogFilters): String {
    val parts = mutableListOf<String>()
    if (query.isNotBlank()) parts += "\"${query.trim()}\""
    activeChips(filters).forEach { parts += it.label }
    return parts.joinToString(" · ")
}

@Composable
private fun Notice(text: String) {
    val c = R4dioTokens.colors
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = text,
            color = Color(c.dim),
            fontSize = 12.sp,
            fontFamily = MonoFamily,
            letterSpacing = 0.12.em,
            textAlign = TextAlign.Center,
        )
    }
}

/** keeps the filter set across a rotation; CatalogFilters holds sets, which the
 *  saved-state bundle cannot carry on its own. */
val FiltersSaver: Saver<CatalogFilters, Any> = listSaver(
    save = { filtersToList(it) },
    restore = { filtersFromList(it) },
)
