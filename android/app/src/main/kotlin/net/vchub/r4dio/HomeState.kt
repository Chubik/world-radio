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
 */
fun isAllHiddenWarn(playableCount: Int, hiddenCount: Int, scope: String): Boolean =
    playableCount == 0 && hiddenCount > 0 && scope != "favs"
