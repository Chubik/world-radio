use crate::catalog::filter::SearchQuery;
use crate::catalog::station::Station;
use rusqlite::Connection;
use std::collections::HashSet;

fn dedup_stations(stations: Vec<Station>) -> Vec<Station> {
    let mut seen = HashSet::new();
    stations
        .into_iter()
        .filter(|s| {
            let key = (
                s.name.to_lowercase(),
                s.countrycode.to_lowercase(),
                s.codec.to_lowercase(),
                s.bitrate,
            );
            seen.insert(key)
        })
        .collect()
}

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let c = Self { conn };
        c.init_schema()?;
        Ok(c)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS stations (
                stationuuid TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url_resolved TEXT NOT NULL DEFAULT '',
                countrycode TEXT NOT NULL DEFAULT '',
                language TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '',
                codec TEXT NOT NULL DEFAULT '',
                bitrate INTEGER NOT NULL DEFAULT 0,
                geo_lat REAL,
                geo_long REAL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS stations_fts
                USING fts5(stationuuid UNINDEXED, name, tags);",
        )?;
        Ok(())
    }

    pub fn upsert(&self, stations: &[Station]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for code in EXCLUDED_COUNTRYCODES {
            tx.execute("DELETE FROM stations WHERE countrycode = ?1", [code])?;
        }
        tx.execute(
            "DELETE FROM stations_fts WHERE stationuuid NOT IN (SELECT stationuuid FROM stations)",
            [],
        )?;
        for s in stations {
            if is_excluded(s) {
                continue;
            }
            tx.execute(
                "INSERT INTO stations
                    (stationuuid,name,url_resolved,countrycode,language,tags,codec,bitrate,geo_lat,geo_long)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                 ON CONFLICT(stationuuid) DO UPDATE SET
                    name=excluded.name, url_resolved=excluded.url_resolved,
                    countrycode=excluded.countrycode, language=excluded.language,
                    tags=excluded.tags, codec=excluded.codec, bitrate=excluded.bitrate,
                    geo_lat=excluded.geo_lat, geo_long=excluded.geo_long",
                rusqlite::params![
                    s.stationuuid, s.name, s.url_resolved, s.countrycode, s.language,
                    s.tags, s.codec, s.bitrate, s.geo_lat, s.geo_long
                ],
            )?;
            tx.execute(
                "DELETE FROM stations_fts WHERE stationuuid = ?1",
                [&s.stationuuid],
            )?;
            tx.execute(
                "INSERT INTO stations_fts (stationuuid,name,tags) VALUES (?1,?2,?3)",
                rusqlite::params![s.stationuuid, s.name, s.tags],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn search_name(&self, term: &str) -> anyhow::Result<Vec<Station>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.stationuuid,s.name,s.url_resolved,s.countrycode,s.language,
                    s.tags,s.codec,s.bitrate,s.geo_lat,s.geo_long
             FROM stations s
             WHERE s.stationuuid IN (
                 SELECT stationuuid FROM stations_fts WHERE stations_fts MATCH ?1
             )
             ORDER BY s.name",
        )?;
        let rows = stmt.query_map([term], row_to_station)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(dedup_stations(out))
    }

    pub fn list_all(&self) -> anyhow::Result<Vec<Station>> {
        let mut stmt = self.conn.prepare(
            "SELECT stationuuid,name,url_resolved,countrycode,language,
                    tags,codec,bitrate,geo_lat,geo_long
             FROM stations
             ORDER BY name",
        )?;
        let rows = stmt.query_map([], row_to_station)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(dedup_stations(out))
    }

    pub fn search(&self, q: &SearchQuery) -> anyhow::Result<Vec<Station>> {
        let mut sql = String::from(
            "SELECT stationuuid, name, url_resolved, countrycode, language, tags, codec, bitrate, geo_lat, geo_long FROM stations",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut where_parts: Vec<String> = Vec::new();

        if let Some(name) = q.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(fts) = fts_prefix_query(name) {
                where_parts.push(
                    "stationuuid IN (SELECT stationuuid FROM stations_fts WHERE stations_fts MATCH ?)"
                        .to_string(),
                );
                params.push(Box::new(fts));
            }
        }
        if let Some(country) = &q.countrycode {
            if !country.is_empty() {
                where_parts.push("countrycode = ?".to_string());
                params.push(Box::new(country.clone()));
            }
        }
        if let Some(lang) = &q.language {
            if !lang.is_empty() {
                where_parts.push("language = ?".to_string());
                params.push(Box::new(lang.clone()));
            }
        }
        if let Some(codec) = &q.codec {
            if !codec.is_empty() {
                where_parts.push("codec = ?".to_string());
                params.push(Box::new(codec.clone()));
            }
        }
        if let Some(tag) = &q.tag {
            if !tag.is_empty() {
                where_parts.push("tags LIKE ?".to_string());
                params.push(Box::new(format!("%{tag}%")));
            }
        }
        if let Some(min) = q.bitrate_min {
            where_parts.push("bitrate >= ?".to_string());
            params.push(Box::new(min));
        }

        if !where_parts.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_parts.join(" AND "));
        }
        sql.push_str(" ORDER BY name");

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_station)?;
        let stations: Result<Vec<_>, _> = rows.collect();
        Ok(dedup_stations(stations?))
    }

    pub fn facets(&self, limit: usize) -> anyhow::Result<crate::catalog::Facets> {
        let countries = self.facet_column("countrycode", limit)?;
        let codecs = self.facet_column("codec", limit)?;
        let tags = self.facet_tags(limit)?;
        Ok(crate::catalog::Facets {
            countries,
            codecs,
            tags,
        })
    }

    fn facet_column(&self, column: &str, limit: usize) -> anyhow::Result<Vec<(String, u32)>> {
        let sql = format!(
            "SELECT {column}, COUNT(*) AS c FROM stations WHERE {column} != '' GROUP BY {column} ORDER BY c DESC, {column} ASC LIMIT ?"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([limit as i64], |r| {
            let v: String = r.get(0)?;
            let c: i64 = r.get(1)?;
            Ok((v, c as u32))
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }

    fn facet_tags(&self, limit: usize) -> anyhow::Result<Vec<(String, u32)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tags FROM stations WHERE tags != ''")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for raw in rows {
            let tags = raw?;
            for tag in tags.split(',') {
                let t = tag.trim();
                if !t.is_empty() {
                    *counts.entry(t.to_string()).or_insert(0) += 1;
                }
            }
        }
        let mut sorted: Vec<(String, u32)> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        sorted.truncate(limit);
        Ok(sorted)
    }

    pub fn get_by_uuid(&self, uuid: &str) -> anyhow::Result<Option<Station>> {
        let mut stmt = self.conn.prepare(
            "SELECT stationuuid,name,url_resolved,countrycode,language,
                    tags,codec,bitrate,geo_lat,geo_long
             FROM stations
             WHERE stationuuid = ?1",
        )?;
        let mut rows = stmt.query_map([uuid], row_to_station)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let c = Self { conn };
        c.init_schema()?;
        c.purge_excluded()?;
        Ok(c)
    }

    pub fn purge_excluded(&self) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for code in EXCLUDED_COUNTRYCODES {
            tx.execute("DELETE FROM stations WHERE countrycode = ?1", [code])?;
        }
        for needle in EXCLUDED_NAME_SUBSTRINGS {
            let pattern = format!("%{needle}%");
            tx.execute(
                "DELETE FROM stations WHERE LOWER(name) LIKE ?1 OR LOWER(tags) LIKE ?1",
                [pattern],
            )?;
        }
        tx.execute(
            "DELETE FROM stations_fts WHERE stationuuid NOT IN (SELECT stationuuid FROM stations)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}

const EXCLUDED_COUNTRYCODES: &[&str] = &["RU", "BY"];
const EXCLUDED_NAME_SUBSTRINGS: &[&str] = &[
    "russia",
    "russian",
    "moscow",
    "moskva",
    "kremlin",
    "putin",
    "россия",
    "русск",
    "москв",
    "kreml",
    "беларус",
    "belarus",
    "минск",
    "minsk",
];

fn fts_prefix_query(input: &str) -> Option<String> {
    let tokens: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.to_lowercase()))
        .collect();
    match tokens.is_empty() {
        true => None,
        false => Some(tokens.join(" ")),
    }
}

fn is_excluded(station: &Station) -> bool {
    if EXCLUDED_COUNTRYCODES
        .iter()
        .any(|c| station.countrycode.eq_ignore_ascii_case(c))
    {
        return true;
    }
    let haystack = format!("{} {}", station.name, station.tags).to_lowercase();
    EXCLUDED_NAME_SUBSTRINGS
        .iter()
        .any(|needle| haystack.contains(needle))
}

fn row_to_station(r: &rusqlite::Row) -> rusqlite::Result<Station> {
    Ok(Station {
        stationuuid: r.get(0)?,
        name: r.get(1)?,
        url_resolved: r.get(2)?,
        countrycode: r.get(3)?,
        language: r.get(4)?,
        tags: r.get(5)?,
        codec: r.get(6)?,
        bitrate: r.get(7)?,
        geo_lat: r.get(8)?,
        geo_long: r.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rich_station(
        uuid: &str,
        name: &str,
        country: &str,
        tags: &str,
        codec: &str,
        bitrate: u32,
    ) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: name.into(),
            url_resolved: String::new(),
            countrycode: country.into(),
            language: String::new(),
            tags: tags.into(),
            codec: codec.into(),
            bitrate,
            geo_lat: None,
            geo_long: None,
        }
    }

    fn station(uuid: &str, name: &str) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: name.into(),
            url_resolved: String::new(),
            countrycode: String::new(),
            language: String::new(),
            tags: String::new(),
            codec: String::new(),
            bitrate: 0,
            geo_lat: None,
            geo_long: None,
        }
    }

    #[test]
    fn dedup_collapses_same_name_country_codec_bitrate() {
        let input = vec![
            rich_station("u1", "CYBERStacja", "PL", "", "MP3", 192),
            rich_station("u2", "CYBERStacja", "PL", "", "MP3", 192),
            rich_station("u3", "CRnet Hits (128)", "US", "", "MP3", 128),
            rich_station("u4", "CRnet Hits (32)", "US", "", "MP3", 32),
        ];
        let out = dedup_stations(input);
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["CYBERStacja", "CRnet Hits (128)", "CRnet Hits (32)"]
        );
    }

    #[test]
    fn dedup_keeps_same_name_different_bitrate() {
        let input = vec![
            rich_station("u1", "Cafe", "GR", "", "MP3", 96),
            rich_station("u2", "Cafe", "GR", "", "MP3", 320),
        ];
        assert_eq!(dedup_stations(input).len(), 2);
    }

    #[test]
    fn upsert_then_fts_search_finds_station() {
        let c = Cache::open_in_memory().unwrap();
        c.upsert(&[station("u1", "Smooth Jazz FM"), station("u2", "Rock Radio")])
            .unwrap();
        let found = c.search_name("jazz").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].stationuuid, "u1");
    }

    #[test]
    fn list_all_returns_all_ordered_by_name() {
        let c = Cache::open_in_memory().unwrap();
        c.upsert(&[station("u2", "Beta"), station("u1", "Alpha")])
            .unwrap();
        let all = c.list_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "Alpha");
        assert_eq!(all[1].name, "Beta");
    }

    #[test]
    fn get_by_uuid_returns_station_when_present_and_none_otherwise() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&[station("u1", "Jazz Live")]).unwrap();

        let found = cache.get_by_uuid("u1").unwrap();
        assert_eq!(found.map(|s| s.name), Some("Jazz Live".to_string()));

        let missing = cache.get_by_uuid("nope").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn upsert_is_idempotent_on_uuid() {
        let c = Cache::open_in_memory().unwrap();
        c.upsert(&[station("u1", "Jazz One")]).unwrap();
        c.upsert(&[station("u1", "Jazz One Renamed")]).unwrap();
        let found = c.search_name("jazz").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Jazz One Renamed");
    }

    #[test]
    fn search_with_only_name_matches_fts() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "Jazz Live", "GB", "", "MP3", 128),
                rich_station("u2", "Rock Hour", "GB", "", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            name: Some("\"jazz\"".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(uuids, vec!["u1"]);
    }

    #[test]
    fn search_with_only_country_filters() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "Alpha", "GB", "", "MP3", 128),
                rich_station("u2", "Beta", "DE", "", "MP3", 128),
                rich_station("u3", "Gamma", "GB", "", "AAC", 96),
            ])
            .unwrap();
        let q = SearchQuery {
            countrycode: Some("GB".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let mut uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.clone()).collect();
        uuids.sort();
        assert_eq!(uuids, vec!["u1", "u3"]);
    }

    #[test]
    fn search_with_name_and_country_intersects() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "Jazz Live", "GB", "", "MP3", 128),
                rich_station("u2", "Jazz Cafe", "DE", "", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            name: Some("\"jazz\"".into()),
            countrycode: Some("GB".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(uuids, vec!["u1"]);
    }

    #[test]
    fn search_with_bitrate_min_filters_lower_bound() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "", "", "MP3", 64),
                rich_station("u2", "B", "", "", "MP3", 128),
                rich_station("u3", "C", "", "", "MP3", 256),
            ])
            .unwrap();
        let q = SearchQuery {
            bitrate_min: Some(128),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let mut uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.clone()).collect();
        uuids.sort();
        assert_eq!(uuids, vec!["u2", "u3"]);
    }

    #[test]
    fn search_with_tag_uses_like_substring() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "", "jazz,smooth", "MP3", 128),
                rich_station("u2", "B", "", "rock", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            tag: Some("jazz".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(uuids, vec!["u1"]);
    }

    #[test]
    fn search_with_bare_word_name_works() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "Jazz Live", "GB", "", "MP3", 128),
                rich_station("u2", "Rock Hour", "GB", "", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            name: Some("jazz".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(uuids, vec!["u1"]);
    }

    #[test]
    fn search_trims_whitespace_in_name() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[rich_station("u1", "Jazz Live", "GB", "", "MP3", 128)])
            .unwrap();
        let q = SearchQuery {
            name: Some("  jazz  ".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(uuids, vec!["u1"]);
    }

    #[test]
    fn facets_returns_top_countries_with_counts() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "GB", "", "MP3", 128),
                rich_station("u2", "B", "GB", "", "MP3", 128),
                rich_station("u3", "C", "GB", "", "MP3", 128),
                rich_station("u4", "D", "DE", "", "MP3", 128),
                rich_station("u5", "E", "US", "", "MP3", 128),
            ])
            .unwrap();
        let f = cache.facets(10).unwrap();
        assert_eq!(
            f.countries,
            vec![("GB".into(), 3), ("DE".into(), 1), ("US".into(), 1)]
        );
    }

    #[test]
    fn facets_returns_top_codecs_with_counts() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "", "", "MP3", 128),
                rich_station("u2", "B", "", "", "MP3", 128),
                rich_station("u3", "C", "", "", "AAC", 128),
            ])
            .unwrap();
        let f = cache.facets(10).unwrap();
        assert_eq!(f.codecs, vec![("MP3".into(), 2), ("AAC".into(), 1)]);
    }

    #[test]
    fn facets_splits_tags_and_counts_individually() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "", "jazz,smooth", "MP3", 128),
                rich_station("u2", "B", "", "jazz,electronic", "MP3", 128),
            ])
            .unwrap();
        let f = cache.facets(10).unwrap();
        let map: std::collections::HashMap<_, _> = f.tags.into_iter().collect();
        assert_eq!(map.get("jazz"), Some(&2));
        assert_eq!(map.get("smooth"), Some(&1));
        assert_eq!(map.get("electronic"), Some(&1));
    }

    #[test]
    fn facets_respects_limit() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "GB", "jazz,rock,pop", "MP3", 128),
                rich_station("u2", "B", "DE", "jazz,electronic,ambient", "AAC", 128),
                rich_station("u3", "C", "US", "rock,country,folk", "OGG", 128),
            ])
            .unwrap();
        let f = cache.facets(2).unwrap();
        assert_eq!(f.countries.len(), 2);
        assert_eq!(f.codecs.len(), 2);
        assert_eq!(f.tags.len(), 2);
    }

    #[test]
    fn fts_prefix_query_wraps_tokens_with_star() {
        assert_eq!(fts_prefix_query("80"), Some("\"80\"*".to_string()));
        assert_eq!(
            fts_prefix_query("smooth jazz"),
            Some("\"smooth\"* \"jazz\"*".to_string())
        );
        assert_eq!(fts_prefix_query("  "), None);
        assert_eq!(fts_prefix_query("80's"), Some("\"80\"* \"s\"*".to_string()));
    }

    #[test]
    fn search_prefix_matches_80s_when_typing_80() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "80s Super Dancefloor", "ES", "80's,pop", "AAC", 192),
                rich_station("u2", "Pure Jazz", "GB", "jazz", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            name: Some("80".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        let uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(uuids, vec!["u1"]);
    }

    #[test]
    fn search_prefix_matches_tag_too() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "Generic FM", "GB", "80s,disco", "MP3", 128),
                rich_station("u2", "Other FM", "GB", "rock", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            name: Some("disco".into()),
            ..Default::default()
        };
        let rows = cache.search(&q).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stationuuid, "u1");
    }
}
