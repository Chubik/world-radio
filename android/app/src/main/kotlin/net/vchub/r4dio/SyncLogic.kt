package net.vchub.r4dio

/**
 * the sync-screen rules that do not need a view, so they can be tested without
 * one. the activity holds the android pieces (pickers, camera, clipboard); this
 * holds the decisions.
 */

/** a key looks like `r4-` followed by lowercase base32. */
fun isSyncKey(text: String?): Boolean {
    val trimmed = text?.trim().orEmpty()
    return trimmed.startsWith("r4-") && trimmed.length > 3
}

/**
 * what a scan produced. a cancelled scan is not a failure — backing out of the
 * camera is how someone changes their mind — so it is its own case rather than
 * an error message.
 */
sealed interface ScanOutcome {
    data class Linked(val key: String) : ScanOutcome
    data object Cancelled : ScanOutcome
    data object NotAKey : ScanOutcome
}

fun scanOutcome(contents: String?): ScanOutcome = when {
    contents == null -> ScanOutcome.Cancelled
    isSyncKey(contents) -> ScanOutcome.Linked(contents.trim())
    else -> ScanOutcome.NotAKey
}

/**
 * the countries offered when hiding some. deliberately a curated forty rather
 * than all 240 the catalogue carries: a list nobody scrolls to the end of is a
 * list nobody uses, and these are the ones with enough stations to matter.
 */
val OFFERED_COUNTRY_CODES: List<String> = listOf(
    "AR", "AT", "AU", "BE", "BR", "CA", "CH", "CL", "CN", "CO",
    "CZ", "DE", "DK", "EG", "ES", "FI", "FR", "GB", "GR", "HU",
    "ID", "IE", "IL", "IN", "IT", "JP", "KR", "MX", "NL", "NO",
    "NZ", "PL", "PT", "RO", "SE", "TH", "TR", "UA", "US", "ZA",
).sorted()

// deliberately no key mask here. the android screen shows the key in full on the
// device that owns it, because that is how it gets typed into a second device;
// the macos client masks it because it renders the key beside a QR that already
// carries it. adding a third format would be a third thing to drift.
