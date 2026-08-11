package net.vchub.r4dio

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * this repo's recurring defect is not wrong logic, it is correct logic nothing
 * ever reaches: [FavStore.setScope] stamps the profile perfectly, and the scope
 * button still never published it because its handler simply did not call
 * `syncNow()`. no unit test of the pure helpers can see that — the wiring lives
 * in a [PlaybackService] inner class that needs the android framework to build.
 *
 * so this asserts the wiring itself: every session command that edits something
 * the account carries must publish before its handler ends.
 */
class AccountPublishTest {
    private val source: String by lazy {
        val here = File("").absoluteFile
        val roots = generateSequence(here) { it.parentFile }
        val file = roots
            .map { File(it, "app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt") }
            .firstOrNull { it.isFile }
        requireNotNull(file) { "cannot locate PlaybackService.kt from $here" }.readText()
    }

    /** the body of one `CMD_X -> { ... }` arm of onCustomCommand. */
    private fun handlerBody(command: String): String {
        val start = source.indexOf("$command -> {")
        assertTrue("no $command arm in onCustomCommand", start >= 0)
        var depth = 0
        var i = source.indexOf('{', start)
        val open = i
        while (i < source.length) {
            when (source[i]) {
                '{' -> depth++
                '}' -> {
                    depth--
                    if (depth == 0) return source.substring(open, i + 1)
                }
            }
            i++
        }
        throw AssertionError("unbalanced braces in the $command arm")
    }

    @Test
    fun starring_a_station_publishes_to_the_account() {
        assertTrue(handlerBody("CMD_STAR").contains("syncNow()"))
    }

    // the scope is the one profile field android can edit. without this call the
    // phone stamps the change and no other device ever hears about it, which is
    // the whole feature being false for the field that matters most here.
    @Test
    fun toggling_the_scope_publishes_to_the_account() {
        assertTrue(handlerBody("CMD_SCOPE").contains("syncNow()"))
    }
}
