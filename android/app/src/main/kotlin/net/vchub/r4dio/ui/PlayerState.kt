package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.EXTRA_CATALOG_GROWING
import net.vchub.r4dio.EXTRA_CATALOG_LOADED
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import net.vchub.r4dio.EXTRA_FAV
import net.vchub.r4dio.EXTRA_FAV_COUNT
import net.vchub.r4dio.EXTRA_FILTER_COUNTRIES
import net.vchub.r4dio.EXTRA_HIDDEN_COUNT
import net.vchub.r4dio.EXTRA_PLAYABLE_COUNT
import net.vchub.r4dio.EXTRA_SCOPE

/**
 * everything the screens read, in one immutable value. replaces the eleven
 * private fields MainActivity used to hold: four tabs cannot each keep their
 * own copy and stay in agreement.
 */
data class UiState(
    val stationName: String = "",
    val country: String = "",
    val codec: String = "",
    val isPlaying: Boolean = false,
    val isFav: Boolean = false,
    val scope: String = "all",
    val favCount: Int = 0,
    val hiddenCount: Int = 0,
    val playableCount: Int = 0,
    val catalogueSize: Int = 0,
    val catalogueGrowing: Boolean = false,
    val catalogLoaded: Boolean = false,
    val filterCountries: List<String> = emptyList(),
)

/**
 * [previous] is carried, not defaulted: extras and player metadata arrive on
 * two separate callbacks, so folding extras over a fresh UiState would blank
 * the station name every time a count changed.
 */
fun uiStateFromExtras(extras: Bundle, previous: UiState): UiState = previous.copy(
    isFav = extras.getBoolean(EXTRA_FAV, previous.isFav),
    scope = extras.getString(EXTRA_SCOPE) ?: previous.scope,
    favCount = extras.getInt(EXTRA_FAV_COUNT, previous.favCount),
    hiddenCount = extras.getInt(EXTRA_HIDDEN_COUNT, previous.hiddenCount),
    playableCount = extras.getInt(EXTRA_PLAYABLE_COUNT, previous.playableCount),
    catalogueSize = extras.getInt(EXTRA_CATALOG_SIZE, previous.catalogueSize),
    catalogueGrowing = extras.getBoolean(EXTRA_CATALOG_GROWING, previous.catalogueGrowing),
    catalogLoaded = extras.getBoolean(EXTRA_CATALOG_LOADED, previous.catalogLoaded),
    filterCountries = extras.getStringArray(EXTRA_FILTER_COUNTRIES)?.toList()
        ?: previous.filterCountries,
)
