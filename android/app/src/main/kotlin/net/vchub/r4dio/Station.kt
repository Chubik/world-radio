package net.vchub.r4dio

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ApiStation(
    val stationuuid: String = "",
    val name: String = "",
    @SerialName("url_resolved") val urlResolved: String = "",
    val countrycode: String = "",
    val codec: String = "",
    val bitrate: Int = 0,
    val tags: String = "",
    val language: String = "",
)

data class Station(
    val uuid: String,
    val name: String,
    val url: String,
    val country: String,
    val codec: String,
    val bitrate: Int,
    val tags: String = "",
    val language: String = "",
)

/** the api sends tags as one comma-separated string; every consumer wants a list. */
fun Station.genres(): List<String> =
    tags.split(",").map { it.trim().lowercase() }.filter { it.isNotEmpty() }

fun ApiStation.toStation(): Station =
    Station(stationuuid, name, urlResolved, countrycode, codec, bitrate, tags, language)

/**
 * the on-disk and over-the-wire shape of a station. every field carries a
 * default on purpose: this decodes the whole ~59k catalogue in one go, and
 * without them a single station missing a codec would fail the entire parse and
 * leave the app with nothing. a station that arrives without a uuid or a url is
 * dropped later by allowedStation, not here.
 */
@Serializable
data class FavStation(
    val uuid: String = "",
    val name: String = "",
    val url: String = "",
    val country: String = "",
    val codec: String = "",
    val bitrate: Int = 0,
    val tags: String = "",
    val language: String = "",
) {
    fun toStation(): Station = Station(uuid, name, url, country, codec, bitrate, tags, language)

    companion object {
        fun of(s: Station): FavStation =
            FavStation(s.uuid, s.name, s.url, s.country, s.codec, s.bitrate, s.tags, s.language)
    }
}
