package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayHistoryTest {
    private fun st(uuid: String, name: String = "Station $uuid") =
        Station(uuid, name, "http://x/$uuid", "UA", "MP3", 128)

    @Test
    fun the_newest_play_is_first() {
        var held = PlayHistory.append(emptyList(), st("a"), 100)
        held = PlayHistory.append(held, st("b"), 200)
        assertEquals(listOf("b", "a"), held.map { it.station.uuid })
    }

    // a history listing the same station eleven times answers "what was that
    // station i heard" far worse than one listing eleven different ones.
    @Test
    fun replaying_a_station_moves_it_up_rather_than_repeating_it() {
        var held = PlayHistory.append(emptyList(), st("a"), 100)
        held = PlayHistory.append(held, st("b"), 200)
        held = PlayHistory.append(held, st("a"), 300)
        assertEquals(listOf("a", "b"), held.map { it.station.uuid })
        assertEquals(2, held.size)
    }

    @Test
    fun a_replay_carries_the_newer_time() {
        var held = PlayHistory.append(emptyList(), st("a"), 100)
        held = PlayHistory.append(held, st("a"), 300)
        assertEquals(300, held.single().at)
    }

    // the station is stored whole, so a row renders and replays without the
    // catalogue — which is the entire reason this is not the sync queue.
    @Test
    fun the_whole_station_is_kept_not_just_its_id() {
        val held = PlayHistory.append(emptyList(), st("a", "Radio Trek"), 100)
        val entry = held.single().station
        assertEquals("Radio Trek", entry.name)
        assertEquals("http://x/a", entry.url)
        assertEquals("UA", entry.country)
        assertEquals("MP3", entry.codec)
        assertEquals(128, entry.bitrate)
    }

    @Test
    fun the_list_stops_growing_at_the_cap() {
        var held = emptyList<HistoryEntry>()
        for (i in 1..PLAY_HISTORY_CAP + 40) {
            held = PlayHistory.append(held, st("s$i"), i.toLong())
        }
        assertEquals(PLAY_HISTORY_CAP, held.size)
        // the oldest are the ones dropped
        assertEquals("s${PLAY_HISTORY_CAP + 40}", held.first().station.uuid)
        assertTrue(held.none { it.station.uuid == "s1" })
    }

    // a play stamped earlier than one already held must not jump the queue —
    // clocks move backwards across devices and restarts.
    @Test
    fun an_out_of_order_stamp_lands_in_its_place() {
        var held = PlayHistory.append(emptyList(), st("late"), 500)
        held = PlayHistory.append(held, st("early"), 100)
        assertEquals(listOf("late", "early"), held.map { it.station.uuid })
    }

    @Test
    fun stations_renders_newest_first() {
        var held = PlayHistory.append(emptyList(), st("a"), 100)
        held = PlayHistory.append(held, st("b"), 200)
        assertEquals(listOf("b", "a"), PlayHistory.stations(held).map { it.uuid })
    }

    @Test
    fun an_empty_history_renders_nothing() {
        assertTrue(PlayHistory.stations(emptyList()).isEmpty())
    }

    @Test
    fun a_blocked_station_is_named_from_the_catalogue() {
        val out = blockedStations(setOf("a"), listOf(st("a", "Radio Trek"), st("b", "Other")))
        assertEquals(listOf("Radio Trek"), out.map { it.name })
    }

    // this screen is the only place a block can be undone, so a station the
    // catalogue no longer carries must still get a row — hiding it would make
    // the block permanent.
    @Test
    fun a_blocked_station_missing_from_the_catalogue_still_gets_a_row() {
        val out = blockedStations(setOf("gone"), listOf(st("a", "Radio Trek")))
        assertEquals(1, out.size)
        assertEquals("gone", out.single().uuid)
        assertEquals("gone", out.single().name)
    }

    // a blocked station is often absent from the catalogue precisely because it
    // was blocked, and a starred one keeps its full record in the fav cache.
    @Test
    fun favourites_are_searched_for_a_name_too() {
        val out = blockedStations(
            blocked = setOf("f"),
            catalogue = emptyList(),
            favourites = listOf(st("f", "Starred Station")),
        )
        assertEquals(listOf("Starred Station"), out.map { it.name })
    }

    @Test
    fun blocked_rows_are_alphabetical() {
        val out = blockedStations(
            setOf("a", "b", "c"),
            listOf(st("a", "Zebra"), st("b", "Alpha"), st("c", "Middle")),
        )
        assertEquals(listOf("Alpha", "Middle", "Zebra"), out.map { it.name })
    }

    @Test
    fun nothing_blocked_renders_nothing() {
        assertTrue(blockedStations(emptySet(), listOf(st("a"))).isEmpty())
    }

    // the same uuid in both lists must produce one row, not two.
    @Test
    fun a_station_in_both_lists_appears_once() {
        val out = blockedStations(
            blocked = setOf("a"),
            catalogue = listOf(st("a", "From Catalogue")),
            favourites = listOf(st("a", "From Favourites")),
        )
        assertEquals(1, out.size)
        // the fav copy wins: it is the record the user actually kept
        assertEquals("From Favourites", out.single().name)
    }
}
