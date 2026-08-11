package net.vchub.r4dio

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfileSyncTest {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = false }

    private fun encode(data: SyncData) = json.encodeToString(SyncData.serializer(), data)

    @Test
    fun a_never_touched_device_sends_the_pre_profile_payload_verbatim() {
        val out = SyncProfile().outgoing(
            favs = listOf("f1"),
            blocked = emptyList(),
            excluded = emptyList(),
            plays = emptyList(),
        )
        assertEquals("""{"favs":["f1"],"blocked":[],"excluded_countries":[]}""", encode(out))
    }

    @Test
    fun a_touched_profile_publishes_the_agreed_shapes() {
        val p = SyncProfile(
            countries = listOf("UA"),
            countriesAt = 10,
            scope = "recent",
            scopeAt = 20,
            theme = "nord",
            themeAt = 30,
        )
        val out = p.outgoing(
            favs = emptyList(),
            blocked = emptyList(),
            excluded = emptyList(),
            plays = listOf(HistoryRecord("s1", 40, false)),
        )
        val body = encode(out)
        assertTrue(body, body.contains("""ffle_filter":{"value":{"countries":["UA"]},"at":10}"""))
        assertTrue(body, body.contains(""""scope":{"value":"recent","at":20}"""))
        assertTrue(body, body.contains(""""theme":{"value":"nord","at":30}"""))
        assertTrue(body, body.contains(""""history":[{"id":"s1","at":40,"gone":false}]"""))
    }

    @Test
    fun an_empty_countries_list_still_publishes_once_stamped() {
        // clearing the filter is a real change; omitting it would let a stale
        // remote list come back on the next sync.
        val p = SyncProfile(countries = emptyList(), countriesAt = 20)
        val out = p.outgoing(emptyList(), emptyList(), emptyList(), emptyList())
        assertTrue(encode(out).contains(""""shuffle_filter":{"value":{"countries":[]},"at":20}"""))
    }

    @Test
    fun a_response_without_the_new_keys_changes_nothing_locally() {
        val server = json.decodeFromString(SyncData.serializer(), """{"favs":[],"blocked":[]}""")
        val p = SyncProfile(countries = listOf("UA"), countriesAt = 5, scope = "favorites", scopeAt = 5)
        val applied = p.applyRemote(server)
        assertEquals(p, applied)
        assertTrue(server.history.isEmpty())
    }

    @Test
    fun newer_remote_filter_replaces_local() {
        val p = SyncProfile(countries = listOf("UA"), countriesAt = 10)
        val applied = p.applyRemote(dataWith(filter = lwwCountries(listOf("PL"), 20)))
        assertEquals(listOf("PL"), applied.countries)
        assertEquals(20L, applied.countriesAt)
    }

    @Test
    fun older_remote_filter_is_ignored() {
        val p = SyncProfile(countries = listOf("UA"), countriesAt = 20)
        val applied = p.applyRemote(dataWith(filter = lwwCountries(listOf("PL"), 10)))
        assertEquals(listOf("UA"), applied.countries)
        assertEquals(20L, applied.countriesAt)
    }

    @Test
    fun a_malformed_remote_filter_is_ignored() {
        val p = SyncProfile(countries = listOf("UA"), countriesAt = 1)
        val applied = p.applyRemote(dataWith(filter = Lww(JsonPrimitive("not-an-object"), 99)))
        assertEquals(listOf("UA"), applied.countries)
        assertEquals(1L, applied.countriesAt)
    }

    @Test
    fun newer_remote_scope_and_theme_replace_local() {
        val p = SyncProfile(scope = "all", scopeAt = 10, theme = "amber-crt", themeAt = 10)
        val applied = p.applyRemote(
            dataWith(scope = Lww(JsonPrimitive("favorites"), 20), theme = Lww(JsonPrimitive("nord"), 20)),
        )
        assertEquals("favorites", applied.scope)
        assertEquals(20L, applied.scopeAt)
        assertEquals("nord", applied.theme)
        assertEquals(20L, applied.themeAt)
    }

    // a theme this build cannot render still has to round-trip untouched, or the
    // device would drag every other device off the theme the user chose there.
    @Test
    fun an_unknown_theme_is_stored_and_re_published() {
        val p = SyncProfile().applyRemote(dataWith(theme = Lww(JsonPrimitive("solarized"), 7)))
        assertEquals("solarized", p.theme)
        val body = encode(p.outgoing(emptyList(), emptyList(), emptyList(), emptyList()))
        assertTrue(body, body.contains(""""theme":{"value":"solarized","at":7}"""))
    }

    @Test
    fun local_scope_maps_onto_the_wire_words() {
        assertEquals("all", wireScope(Scope.ALL))
        assertEquals("favorites", wireScope(Scope.FAVS))
    }

    @Test
    fun the_wire_words_parse_back_to_the_local_scope() {
        assertEquals(Scope.ALL, localScope("all"))
        assertEquals(Scope.FAVS, localScope("favorites"))
    }

    @Test
    fun legacy_uppercase_scopes_still_parse() {
        assertEquals(Scope.ALL, localScope("ALL"))
        assertEquals(Scope.FAVS, localScope("FAVS"))
    }

    // recent/blocked/dead exist on the desktop and android cannot show them. it
    // must leave its own scope alone rather than approximate them as ALL.
    @Test
    fun a_scope_android_cannot_represent_leaves_the_local_scope_untouched() {
        assertNull(localScope("recent"))
        assertNull(localScope("blocked"))
        assertNull(localScope("dead"))
        assertNull(localScope("nonsense"))
        assertNull(localScope(""))
    }

    // and the stamp still advances, so the device stops re-publishing a scope
    // the user has since changed elsewhere — but it re-publishes the desktop's
    // own word, never a substituted one.
    @Test
    fun an_unrepresentable_remote_scope_is_stored_and_re_published_verbatim() {
        val p = SyncProfile(scope = "all", scopeAt = 10)
        val applied = p.applyRemote(dataWith(scope = Lww(JsonPrimitive("dead"), 20)))
        assertEquals("dead", applied.scope)
        assertEquals(20L, applied.scopeAt)
        val body = encode(applied.outgoing(emptyList(), emptyList(), emptyList(), emptyList()))
        assertTrue(body, body.contains(""""scope":{"value":"dead","at":20}"""))
    }

    @Test
    fun setting_the_scope_stamps_the_change() {
        val p = SyncProfile().withScope("favorites", 50)
        assertEquals("favorites", p.scope)
        assertEquals(50L, p.scopeAt)
    }

    // a same-value save must not move the stamp, or an idle device would win
    // every race against a device that actually changed something.
    @Test
    fun setting_the_same_scope_does_not_move_the_stamp() {
        val p = SyncProfile().withScope("favorites", 50).withScope("favorites", 99)
        assertEquals(50L, p.scopeAt)
    }

    // the filter arrives lowercase from a client that did not normalise it; the
    // pick path compares against `station.country.uppercase()`, so a lowercase
    // code stored verbatim would silently match nothing at all.
    @Test
    fun a_remote_filter_is_normalised_on_the_way_in() {
        val p = SyncProfile().applyRemote(dataWith(filter = lwwCountries(listOf("ua", "pl"), 10)))
        assertEquals(listOf("UA", "PL"), p.countries)
    }

    @Test
    fun play_appends_history_record() {
        val queued = HistoryQueue.append(emptySet(), "u1", 100)
        assertEquals(setOf("u1|100"), queued)
        assertEquals(listOf(HistoryRecord("u1", 100, false)), HistoryQueue.records(queued))
    }

    // a station replayed later must push its newer time, not two entries.
    @Test
    fun replaying_a_station_keeps_only_the_newer_stamp() {
        val queued = HistoryQueue.append(HistoryQueue.append(emptySet(), "u1", 100), "u1", 200)
        assertEquals(listOf(HistoryRecord("u1", 200, false)), HistoryQueue.records(queued))
    }

    @Test
    fun an_older_replay_stamp_never_overwrites_a_newer_one() {
        val queued = HistoryQueue.append(HistoryQueue.append(emptySet(), "u1", 200), "u1", 100)
        assertEquals(listOf(HistoryRecord("u1", 200, false)), HistoryQueue.records(queued))
    }

    @Test
    fun a_malformed_queue_entry_is_dropped_rather_than_crashing() {
        assertEquals(emptyList<HistoryRecord>(), HistoryQueue.records(setOf("junk", "u1|notanumber")))
    }

    // the queue is drained by what was actually sent, so a play recorded during
    // the round-trip survives instead of being silently dropped.
    @Test
    fun draining_removes_only_what_was_pushed() {
        val before = setOf("u1|100", "u2|200")
        val after = HistoryQueue.drain(before + "u3|300", HistoryQueue.records(before))
        assertEquals(setOf("u3|300"), after)
    }

    @Test
    fun the_queue_is_capped_at_the_history_cap() {
        var q = emptySet<String>()
        repeat(HISTORY_CAP + 50) { q = HistoryQueue.append(q, "u$it", it.toLong()) }
        assertEquals(HISTORY_CAP, q.size)
        // the newest survive, the oldest fall off
        assertTrue(HistoryQueue.records(q).all { it.at >= 50 })
    }

    @Test
    fun the_push_carries_the_queued_plays() {
        val out = SyncProfile().outgoing(
            favs = emptyList(),
            blocked = emptyList(),
            excluded = emptyList(),
            plays = HistoryQueue.records(setOf("u1|100")),
        )
        assertEquals(listOf(HistoryRecord("u1", 100, false)), out.history)
    }

    private fun lwwCountries(countries: List<String>, at: Long) = Lww(
        buildJsonObject {
            put("countries", kotlinx.serialization.json.JsonArray(countries.map { JsonPrimitive(it) }))
        },
        at,
    )

    private fun dataWith(
        filter: Lww? = null,
        scope: Lww? = null,
        theme: Lww? = null,
        history: List<HistoryRecord> = emptyList(),
    ) = SyncData(
        favs = emptyList(),
        blocked = emptyList(),
        shuffle_filter = filter,
        scope = scope,
        theme = theme,
        history = history,
    )

    @Test
    fun applying_a_remote_profile_reports_nothing_when_nothing_is_newer() {
        val p = SyncProfile(countries = listOf("UA"), countriesAt = 20, scope = "all", scopeAt = 20)
        assertFalse(p.applyRemote(dataWith(filter = lwwCountries(listOf("PL"), 5))) != p)
    }
}
