package net.vchub.r4dio

/**
 * home-screen decisions kept free of android types so they can be unit tested.
 * `scope` is the wire value carried in the session extras: "favs" or "all".
 */

/**
 * favourites bypass the country filter entirely (FavLogic.pickFav ignores
 * userExcluded), so advertising a filter in that scope would be false.
 */
fun showsHiddenPill(hiddenCount: Int, scope: String): Boolean =
    hiddenCount > 0 && scope != "favs"

/**
 * only blame the user's filters when they are actually set — an empty playable
 * set with no hidden countries is a network or catalogue problem, and in favs
 * scope pickForScope falls back to the catalogue, so the filter is not the cause.
 *
 * [catalogLoaded] guards against the cold-start race: the catalogue read/fetch and
 * the sync round-trip run on independent concurrency domains with no ordering, so
 * playableCount can read 0 for a split second on app launch simply because the
 * catalogue load has not resolved yet, not because the filters emptied it. This
 * must track whether a load was *attempted*, not whether it left any stations —
 * the filters-emptied-the-fetch case is a resolved attempt with zero stations,
 * and still needs to warn.
 */
fun isAllHiddenWarn(playableCount: Int, hiddenCount: Int, scope: String, catalogLoaded: Boolean): Boolean =
    catalogLoaded && playableCount == 0 && hiddenCount > 0 && scope != "favs"

/** how many country codes the pill spells out before it summarises the rest. */
private const val FILTER_PILL_CODES = 3

/**
 * the synced shuffle filter, or null when none is set. the row is narrow, so a
 * long list is cut after three codes and the rest counted.
 *
 * shown in favs scope too, even though it does not apply there: hiding it meant a
 * filter the user had set simply disappeared, which read as "my filter is gone"
 * rather than "my filter is not in force right now". [filterIsInForce] is what
 * carries that difference to the screen.
 */
fun filterPillLabel(countries: List<String>, scope: String): String? {
    if (countries.isEmpty()) return null
    val shown = countries.take(FILTER_PILL_CODES).joinToString("·")
    val rest = countries.size - FILTER_PILL_CODES
    return when (rest > 0) {
        true -> "FILTER: $shown +$rest"
        false -> "FILTER: $shown"
    }
}

/**
 * whether the filter actually decides what plays. favourites bypass it entirely
 * (FavLogic.pickFav ignores it, exactly as it ignores excluded countries), so in
 * favs scope a set filter is real but dormant — and the pill has to say so.
 */
fun filterIsInForce(countries: List<String>, scope: String): Boolean =
    countries.isNotEmpty() && scope != "favs"

/**
 * the screen-awake toggle is a car feature: the phone is in a mount, and a screen
 * that blanks mid-drive means fumbling to see what is playing. off by default, so
 * nothing changes for anyone who does not ask for it.
 */
fun keepAwakeLabel(on: Boolean): String = when (on) {
    true -> "☀ AWAKE"
    false -> "☾ SLEEPS"
}

/** the stored flag is the only source of truth — never the window's current state,
 *  which the system may have cleared behind our back. */
fun nextKeepAwake(current: Boolean): Boolean = !current
