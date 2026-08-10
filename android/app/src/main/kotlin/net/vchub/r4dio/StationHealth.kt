package net.vchub.r4dio

// media3 PlaybackException codes, written as literals so this file stays pure
// jvm and the classification is testable without the android framework.
// station-side: 2003 invalid content type, 2004 bad http status, 2005 file not
// found, 3xxx parsing, 4xxx decoding. everything else — including 2001/2002
// network failures and any unknown code — is treated as the device's problem:
// wrongly hiding a live station is worse than meeting a dead one again.
fun shouldBlame(errorCode: Int): Boolean =
    when {
        errorCode in setOf(2003, 2004, 2005) -> true
        errorCode in 3000..3999 -> true
        errorCode in 4000..4999 -> true
        else -> false
    }

class HealthTracker(private val budget: Int = 5) {
    private var blamesSinceSuccess = 0

    fun onError(blame: Boolean): Boolean {
        if (!blame) return false
        if (blamesSinceSuccess >= budget) return false
        blamesSinceSuccess += 1
        return true
    }

    fun onSuccess() {
        blamesSinceSuccess = 0
    }
}
