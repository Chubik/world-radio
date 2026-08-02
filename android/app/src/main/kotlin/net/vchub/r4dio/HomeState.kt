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
 * catalogue has not landed yet, not because the filters emptied it. Only warn once
 * a catalogue actually exists to be emptied.
 */
fun isAllHiddenWarn(playableCount: Int, hiddenCount: Int, scope: String, catalogLoaded: Boolean): Boolean =
    catalogLoaded && playableCount == 0 && hiddenCount > 0 && scope != "favs"
