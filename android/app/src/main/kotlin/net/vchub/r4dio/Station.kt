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
)

data class Station(
    val uuid: String,
    val name: String,
    val url: String,
    val country: String,
    val codec: String,
    val bitrate: Int,
)

fun ApiStation.toStation(): Station =
    Station(stationuuid, name, urlResolved, countrycode, codec, bitrate)

@Serializable
data class FavStation(
    val uuid: String,
    val name: String,
    val url: String,
    val country: String,
    val codec: String,
    val bitrate: Int,
) {
    fun toStation(): Station = Station(uuid, name, url, country, codec, bitrate)

    companion object {
        fun of(s: Station): FavStation =
            FavStation(s.uuid, s.name, s.url, s.country, s.codec, s.bitrate)
    }
}
