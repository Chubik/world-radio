use crate::catalog::filter::SearchQuery;
use crate::catalog::station::Station;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
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

/// countries and codecs are bounded sets, so this is a ceiling rather than a
/// top-N cut: it exists only so a corrupt column cannot return unbounded rows.
const COUNTRY_LIMIT: usize = 1000;

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
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            self.conn.execute_batch(
                "DROP TABLE IF EXISTS stations;
                 DROP TABLE IF EXISTS stations_fts;
                 DROP TABLE IF EXISTS meta;",
            )?;
        }
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
                votes INTEGER NOT NULL DEFAULT 0,
                geo_lat REAL,
                geo_long REAL
            );
            CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE VIRTUAL TABLE IF NOT EXISTS stations_fts
                USING fts5(stationuuid UNINDEXED, name, tags);",
        )?;
        self.conn
            .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
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
            if is_banned(s) {
                continue;
            }
            tx.execute(
                "INSERT INTO stations
                    (stationuuid,name,url_resolved,countrycode,language,tags,codec,bitrate,votes,geo_lat,geo_long)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(stationuuid) DO UPDATE SET
                    name=excluded.name, url_resolved=excluded.url_resolved,
                    countrycode=excluded.countrycode, language=excluded.language,
                    tags=excluded.tags, codec=excluded.codec, bitrate=excluded.bitrate,
                    votes=excluded.votes,
                    geo_lat=excluded.geo_lat, geo_long=excluded.geo_long",
                rusqlite::params![
                    s.stationuuid, s.name, s.url_resolved, s.countrycode, s.language,
                    s.tags, s.codec, s.bitrate, s.votes, s.geo_lat, s.geo_long
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

    pub fn search_name(&self, term: &str, excluded: &[String]) -> anyhow::Result<Vec<Station>> {
        let sql = format!(
            "SELECT s.stationuuid,s.name,s.url_resolved,s.countrycode,s.language,
                    s.tags,s.codec,s.bitrate,s.votes,s.geo_lat,s.geo_long
             FROM stations s
             WHERE s.stationuuid IN (
                 SELECT stationuuid FROM stations_fts WHERE stations_fts MATCH ?
             ){}
             ORDER BY s.name",
            excluded_clause(excluded)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(term.to_string())];
        params.extend(excluded_params(excluded));
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_station)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(dedup_stations(out))
    }

    pub fn list_all(&self, excluded: &[String]) -> anyhow::Result<Vec<Station>> {
        let sql = format!(
            "SELECT stationuuid,name,url_resolved,countrycode,language,
                    tags,codec,bitrate,votes,geo_lat,geo_long
             FROM stations
             WHERE 1=1{}
             ORDER BY name",
            excluded_clause(excluded)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params = excluded_params(excluded);
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_station)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(dedup_stations(out))
    }

    pub fn search(&self, q: &SearchQuery, excluded: &[String]) -> anyhow::Result<Vec<Station>> {
        self.search_limited(q, excluded, None)
    }

    /// a browse list draws at most a couple of hundred rows, but "radio" matches
    /// 26,810 of them and building every one costs seconds. `limit` pushes the
    /// cut into sqlite so those rows are never materialised at all.
    ///
    /// the dedup goes with it: cutting first and deduping after would answer a
    /// single row for a limit of ten whenever the head of the list repeats, so
    /// sqlite groups on the same key `dedup_stations` uses and the limit then
    /// counts distinct stations.
    pub fn search_limited(
        &self,
        q: &SearchQuery,
        excluded: &[String],
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<Station>> {
        self.search_page(q, excluded, limit, 0)
    }

    /// the same search, starting `offset` rows in. paging lives here rather than
    /// in the caller because the cut has to happen after the sort and the dedup
    /// — skipping rows of an already-trimmed page would skip the wrong ones.
    pub fn search_page(
        &self,
        q: &SearchQuery,
        excluded: &[String],
        limit: Option<usize>,
        offset: usize,
    ) -> anyhow::Result<Vec<Station>> {
        let mut sql = String::from(
            "SELECT stationuuid, name, url_resolved, countrycode, language, tags, codec, bitrate, votes, geo_lat, geo_long FROM stations",
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
        // every selected value in a group must reach the sql: keeping only the
        // first one returned stations from one country while the user had
        // several picked.
        let countries: Vec<&String> = q.countrycodes.iter().filter(|c| !c.is_empty()).collect();
        if !countries.is_empty() {
            let marks = vec!["?"; countries.len()].join(",");
            where_parts.push(format!("UPPER(countrycode) IN ({marks})"));
            for c in &countries {
                params.push(Box::new(c.to_uppercase()));
            }
        }
        if let Some(lang) = &q.language {
            if !lang.is_empty() {
                where_parts.push("language = ?".to_string());
                params.push(Box::new(lang.clone()));
            }
        }
        let codecs: Vec<&String> = q.codecs.iter().filter(|c| !c.is_empty()).collect();
        if !codecs.is_empty() {
            let marks = vec!["?"; codecs.len()].join(",");
            where_parts.push(format!("UPPER(codec) IN ({marks})"));
            for c in &codecs {
                params.push(Box::new(c.to_uppercase()));
            }
        }
        let tags: Vec<&String> = q.tags.iter().filter(|t| !t.is_empty()).collect();
        if !tags.is_empty() {
            let ors: Vec<String> = tags.iter().map(|_| "tags LIKE ?".to_string()).collect();
            where_parts.push(format!("({})", ors.join(" OR ")));
            for t in &tags {
                params.push(Box::new(format!("%{t}%")));
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
        if !excluded.is_empty() {
            if where_parts.is_empty() {
                sql.push_str(" WHERE 1=1");
            }
            sql.push_str(&excluded_clause(excluded));
            params.extend(excluded_params(excluded));
        }
        if limit.is_some() {
            // the same key dedup_stations uses, so a bounded page counts distinct
            // stations. min(rowid) keeps the choice of survivor deterministic.
            sql.push_str(
                " GROUP BY lower(name), lower(countrycode), lower(codec), bitrate
                  HAVING rowid = min(rowid)",
            );
        }
        sql.push_str(" ORDER BY ");
        sql.push_str(q.sort.clause());
        if let Some(n) = limit {
            sql.push_str(" LIMIT ?");
            params.push(Box::new(n as i64));
            if offset > 0 {
                sql.push_str(" OFFSET ?");
                params.push(Box::new(offset as i64));
            }
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_station)?;
        let stations: Result<Vec<_>, _> = rows.collect();
        // still deduped in rust: the grouping above only runs on the bounded path.
        Ok(dedup_stations(stations?))
    }

    pub fn facets(&self, tag_limit: usize) -> anyhow::Result<crate::catalog::Facets> {
        // countries and codecs are bounded (a few hundred / a dozen), so list
        // them all — otherwise a top-N cut hides smaller countries entirely.
        // tags run into the thousands, so keep the top-N there.
        let countries = self.facet_column("countrycode", COUNTRY_LIMIT)?;
        let codecs = self.facet_column("codec", COUNTRY_LIMIT)?;
        let tags = self.facet_tags(tag_limit)?;
        Ok(crate::catalog::Facets {
            countries,
            codecs,
            tags,
        })
    }

    /// the country list on its own. `facets` also counts codecs and splits the
    /// tags of all 58k rows, which a country tree never shows and which costs
    /// seconds of the caller's time.
    pub fn country_counts(&self) -> anyhow::Result<Vec<(String, u32)>> {
        self.facet_column("countrycode", COUNTRY_LIMIT)
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
                    tags,codec,bitrate,votes,geo_lat,geo_long
             FROM stations
             WHERE stationuuid = ?1",
        )?;
        let mut rows = stmt.query_map([uuid], row_to_station)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn replace_all(&self, stations: &[Station]) -> anyhow::Result<usize> {
        if stations.is_empty() {
            anyhow::bail!("refusing to replace catalog with an empty dump");
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM stations", [])?;
        tx.execute("DELETE FROM stations_fts", [])?;
        let mut n = 0usize;
        for s in stations {
            if is_banned(s) {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO stations
                    (stationuuid,name,url_resolved,countrycode,language,tags,codec,bitrate,votes,geo_lat,geo_long)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                rusqlite::params![
                    s.stationuuid, s.name, s.url_resolved, s.countrycode, s.language,
                    s.tags, s.codec, s.bitrate, s.votes, s.geo_lat, s.geo_long
                ],
            )?;
            tx.execute(
                "INSERT INTO stations_fts (stationuuid,name,tags) VALUES (?1,?2,?3)",
                rusqlite::params![s.stationuuid, s.name, s.tags],
            )?;
            n += 1;
        }
        tx.commit()?;
        Ok(n)
    }

    pub fn set_last_sync(&self, unix_secs: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key,value) VALUES ('last_sync', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [unix_secs.to_string()],
        )?;
        Ok(())
    }

    pub fn last_sync(&self) -> anyhow::Result<Option<i64>> {
        let v: Option<String> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key='last_sync'", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(v.and_then(|s| s.parse().ok()))
    }

    pub fn count(&self) -> anyhow::Result<usize> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM stations", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    pub fn list_by_popularity(
        &self,
        favourites: &[String],
        limit: usize,
        excluded: &[String],
    ) -> anyhow::Result<Vec<Station>> {
        let excl_clause = excluded_clause(excluded);
        let mut favs = Vec::new();
        if !favourites.is_empty() {
            let placeholders = vec!["?"; favourites.len()].join(",");
            let sql = format!(
                "SELECT stationuuid,name,url_resolved,countrycode,language,tags,codec,bitrate,votes,geo_lat,geo_long
                 FROM stations WHERE stationuuid IN ({placeholders}){excl_clause}
                 ORDER BY votes DESC, name"
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = favourites
                .iter()
                .map(|f| Box::new(f.clone()) as Box<dyn rusqlite::ToSql>)
                .collect();
            params.extend(excluded_params(excluded));
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_station)?;
            for r in rows {
                favs.push(r?);
            }
        }

        let rest_limit = limit.saturating_sub(favs.len());
        let placeholders = vec!["?"; favourites.len()].join(",");
        let sql = format!(
            "SELECT stationuuid,name,url_resolved,countrycode,language,tags,codec,bitrate,votes,geo_lat,geo_long
             FROM stations WHERE stationuuid NOT IN ({placeholders}){excl_clause}
             ORDER BY votes DESC, name LIMIT ?"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = favourites
            .iter()
            .map(|f| Box::new(f.clone()) as Box<dyn rusqlite::ToSql>)
            .collect();
        params.extend(excluded_params(excluded));
        params.push(Box::new(rest_limit as i64));
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_station)?;
        for r in rows {
            favs.push(r?);
        }
        Ok(dedup_stations(favs))
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

const SCHEMA_VERSION: i64 = 1;

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

fn excluded_clause(excluded: &[String]) -> String {
    if excluded.is_empty() {
        return String::new();
    }
    let placeholders = excluded.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    format!(" AND UPPER(countrycode) NOT IN ({placeholders})")
}

fn excluded_params(excluded: &[String]) -> Vec<Box<dyn rusqlite::ToSql>> {
    excluded
        .iter()
        .map(|c| Box::new(c.to_uppercase()) as Box<dyn rusqlite::ToSql>)
        .collect()
}

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

fn is_banned(station: &Station) -> bool {
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

pub fn text_is_excluded(text: &str) -> bool {
    let haystack = text.to_lowercase();
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
        votes: r.get(8)?,
        geo_lat: r.get(9)?,
        geo_long: r.get(10)?,
        // not persisted: the server's verdict is only consumed at ingest time,
        // a row read back from our own cache is never treated as reported-dead.
        lastcheckok: 1,
        lastchecktime_iso8601: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Sort;

    fn bare() -> Station {
        Station {
            stationuuid: String::new(),
            name: String::new(),
            url_resolved: String::new(),
            countrycode: String::new(),
            language: String::new(),
            tags: String::new(),
            codec: String::new(),
            bitrate: 0,
            votes: 0,
            geo_lat: None,
            geo_long: None,
            lastcheckok: 1,
            lastchecktime_iso8601: String::new(),
        }
    }

    // the sort has to reach sqlite: a page is 200 rows out of tens of thousands,
    // and sorting the page afterwards would only order whichever slice the
    // database happened to return first.
    #[test]
    fn sort_orders_the_whole_catalogue_not_just_the_page() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "quiet".into(),
                name: "Zzz Quiet".into(),
                bitrate: 64,
                votes: 1,
                ..bare()
            },
            Station {
                stationuuid: "loud".into(),
                name: "Aaa Loud".into(),
                bitrate: 320,
                votes: 999,
                ..bare()
            },
        ])
        .unwrap();

        let by_name = c
            .search_limited(&SearchQuery::default(), &[], Some(10))
            .unwrap();
        assert_eq!(by_name[0].stationuuid, "loud", "name sort is alphabetical");

        let by_votes = c
            .search_limited(
                &SearchQuery {
                    sort: Sort::Popular,
                    ..Default::default()
                },
                &[],
                Some(10),
            )
            .unwrap();
        assert_eq!(by_votes[0].stationuuid, "loud");

        // "Zzz Quiet" sorts last alphabetically; a bitrate sort must still put
        // the 320k station first, which proves the order came from sql.
        let by_bitrate = c
            .search_limited(
                &SearchQuery {
                    sort: Sort::Bitrate,
                    ..Default::default()
                },
                &[],
                Some(10),
            )
            .unwrap();
        assert_eq!(by_bitrate[0].bitrate, 320);
    }

    #[test]
    fn a_search_returns_every_selected_country() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "a".into(),
                name: "ua one".into(),
                countrycode: "UA".into(),
                ..bare()
            },
            Station {
                stationuuid: "b".into(),
                name: "us one".into(),
                countrycode: "US".into(),
                ..bare()
            },
            Station {
                stationuuid: "c".into(),
                name: "pl one".into(),
                countrycode: "PL".into(),
                ..bare()
            },
        ])
        .unwrap();
        let q = SearchQuery {
            countrycodes: vec!["UA".into(), "US".into()],
            ..Default::default()
        };
        let got = c.search(&q, &[]).unwrap();
        let mut ids: Vec<&str> = got.iter().map(|s| s.stationuuid.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec!["a", "b"],
            "both selected countries must come back"
        );
    }

    #[test]
    fn an_empty_country_list_does_not_restrict() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "a".into(),
                name: "ua one".into(),
                countrycode: "UA".into(),
                ..bare()
            },
            Station {
                stationuuid: "b".into(),
                name: "pl one".into(),
                countrycode: "PL".into(),
                ..bare()
            },
        ])
        .unwrap();
        let got = c.search(&SearchQuery::default(), &[]).unwrap();
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn a_search_returns_every_selected_tag_and_codec() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "a".into(),
                name: "jazz mp3".into(),
                tags: "jazz,smooth".into(),
                codec: "MP3".into(),
                ..bare()
            },
            Station {
                stationuuid: "b".into(),
                name: "rock aac".into(),
                tags: "rock".into(),
                codec: "AAC".into(),
                ..bare()
            },
            Station {
                stationuuid: "c".into(),
                name: "pop ogg".into(),
                tags: "pop".into(),
                codec: "OGG".into(),
                ..bare()
            },
        ])
        .unwrap();
        let q = SearchQuery {
            tags: vec!["jazz".into(), "rock".into()],
            codecs: vec!["MP3".into(), "AAC".into()],
            ..Default::default()
        };
        let got = c.search(&q, &[]).unwrap();
        let mut ids: Vec<&str> = got.iter().map(|s| s.stationuuid.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn search_excludes_user_countries() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "1".into(),
                name: "FR one".into(),
                countrycode: "FR".into(),
                votes: 5,
                ..bare()
            },
            Station {
                stationuuid: "2".into(),
                name: "US one".into(),
                countrycode: "US".into(),
                votes: 9,
                ..bare()
            },
        ])
        .unwrap();
        let all = c.list_all(&["US".to_string()]).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].countrycode, "FR");
        // empty exclude set returns everything (minus the always-on RU/BY at ingest)
        assert_eq!(c.list_all(&[]).unwrap().len(), 2);
    }

    #[test]
    fn replace_all_bans_ru_by_and_counts_allowed() {
        let c = Cache::open_in_memory().unwrap();
        let dump = vec![
            Station {
                stationuuid: "1".into(),
                name: "Jazz FM".into(),
                countrycode: "FR".into(),
                votes: 10,
                ..bare()
            },
            Station {
                stationuuid: "2".into(),
                name: "Radio Moscow".into(),
                countrycode: "US".into(),
                votes: 99,
                ..bare()
            },
            Station {
                stationuuid: "3".into(),
                name: "Any".into(),
                countrycode: "RU".into(),
                votes: 5,
                ..bare()
            },
            Station {
                stationuuid: "4".into(),
                name: "Минск FM".into(),
                countrycode: "PL".into(),
                votes: 7,
                ..bare()
            },
        ];
        let n = c.replace_all(&dump).unwrap();
        assert_eq!(n, 1, "only the FR jazz station survives the ban gate");
        let all = c.list_all(&[]).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].stationuuid, "1");
    }

    #[test]
    fn replace_all_refuses_empty_and_keeps_cache() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[Station {
            stationuuid: "1".into(),
            name: "Keep".into(),
            countrycode: "FR".into(),
            votes: 1,
            ..bare()
        }])
        .unwrap();
        assert!(c.replace_all(&[]).is_err());
        assert_eq!(
            c.list_all(&[]).unwrap().len(),
            1,
            "failed empty sync must not wipe cache"
        );
    }

    #[test]
    fn last_sync_roundtrips() {
        let c = Cache::open_in_memory().unwrap();
        assert_eq!(c.last_sync().unwrap(), None);
        c.set_last_sync(1_700_000_000).unwrap();
        assert_eq!(c.last_sync().unwrap(), Some(1_700_000_000));
    }

    #[test]
    fn list_by_popularity_keeps_low_vote_favourite_beyond_limit() {
        let c = Cache::open_in_memory().unwrap();
        let mut dump = Vec::new();
        for i in 0..20 {
            dump.push(Station {
                stationuuid: format!("p{i}"),
                name: format!("Pop{i}"),
                countrycode: "FR".into(),
                votes: 1000 - i as u64,
                ..bare()
            });
        }
        dump.push(Station {
            stationuuid: "fav".into(),
            name: "NicheFav".into(),
            countrycode: "FR".into(),
            votes: 0,
            ..bare()
        });
        c.replace_all(&dump).unwrap();
        let out = c.list_by_popularity(&["fav".to_string()], 5, &[]).unwrap();
        assert_eq!(
            out[0].stationuuid, "fav",
            "favourite hoisted even though 0 votes and beyond top-5"
        );
        assert!(out.len() <= 6, "roughly limit + hoisted favs");
    }

    #[test]
    fn opening_old_schema_db_recreates_cleanly() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("old.db");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE stations (stationuuid TEXT PRIMARY KEY, name TEXT, bitrate INTEGER);",
            )
            .unwrap();
            // no votes column, no meta table, user_version stays 0
        }
        let c = Cache::open(&path).unwrap(); // must not error
                                             // replace_all + list must work (proves votes column + meta exist now)
        c.replace_all(&[Station {
            stationuuid: "1".into(),
            name: "X".into(),
            countrycode: "FR".into(),
            votes: 5,
            ..bare()
        }])
        .unwrap();
        assert_eq!(c.count().unwrap(), 1);
        assert!(c.last_sync().unwrap().is_none());
    }

    #[test]
    fn list_by_popularity_hoists_favourites_then_votes_desc() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "hi".into(),
                name: "HighVotes".into(),
                countrycode: "FR".into(),
                votes: 100,
                ..bare()
            },
            Station {
                stationuuid: "lo".into(),
                name: "LowVotes".into(),
                countrycode: "FR".into(),
                votes: 1,
                ..bare()
            },
            Station {
                stationuuid: "fav".into(),
                name: "Favourite".into(),
                countrycode: "FR".into(),
                votes: 2,
                ..bare()
            },
        ])
        .unwrap();
        let out = c.list_by_popularity(&["fav".to_string()], 10, &[]).unwrap();
        assert_eq!(
            out[0].stationuuid, "fav",
            "favourite first regardless of votes"
        );
        assert_eq!(out[1].stationuuid, "hi", "then highest votes");
        assert_eq!(out[2].stationuuid, "lo");
    }

    #[test]
    fn list_by_popularity_empty_favourites_returns_popular() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "hi".into(),
                name: "Hi".into(),
                countrycode: "FR".into(),
                votes: 50,
                ..bare()
            },
            Station {
                stationuuid: "lo".into(),
                name: "Lo".into(),
                countrycode: "FR".into(),
                votes: 1,
                ..bare()
            },
        ])
        .unwrap();
        let out = c.list_by_popularity(&[], 10, &[]).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0].stationuuid, "hi",
            "highest votes first when no favourites"
        );
    }

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
            votes: 0,
            geo_lat: None,
            geo_long: None,
            lastcheckok: 1,
            lastchecktime_iso8601: String::new(),
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
            votes: 0,
            geo_lat: None,
            geo_long: None,
            lastcheckok: 1,
            lastchecktime_iso8601: String::new(),
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
        let found = c.search_name("jazz", &[]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].stationuuid, "u1");
    }

    #[test]
    fn search_name_excludes_user_countries() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station {
                stationuuid: "1".into(),
                name: "Jazz FR".into(),
                countrycode: "FR".into(),
                votes: 1,
                ..bare()
            },
            Station {
                stationuuid: "2".into(),
                name: "Jazz US".into(),
                countrycode: "US".into(),
                votes: 1,
                ..bare()
            },
        ])
        .unwrap();
        let out = c.search_name("jazz", &["US".to_string()]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].countrycode, "FR");
    }

    #[test]
    fn list_all_returns_all_ordered_by_name() {
        let c = Cache::open_in_memory().unwrap();
        c.upsert(&[station("u2", "Beta"), station("u1", "Alpha")])
            .unwrap();
        let all = c.list_all(&[]).unwrap();
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
        let found = c.search_name("jazz", &[]).unwrap();
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
        let rows = cache.search(&q, &[]).unwrap();
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
            countrycodes: vec!["GB".into()],
            ..Default::default()
        };
        let rows = cache.search(&q, &[]).unwrap();
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
            countrycodes: vec!["GB".into()],
            ..Default::default()
        };
        let rows = cache.search(&q, &[]).unwrap();
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
        let rows = cache.search(&q, &[]).unwrap();
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
            tags: vec!["jazz".into()],
            ..Default::default()
        };
        let rows = cache.search(&q, &[]).unwrap();
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
        let rows = cache.search(&q, &[]).unwrap();
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
        let rows = cache.search(&q, &[]).unwrap();
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
    fn facets_limit_bounds_tags_but_lists_all_countries_and_codecs() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "GB", "jazz,rock,pop", "MP3", 128),
                rich_station("u2", "B", "DE", "jazz,electronic,ambient", "AAC", 128),
                rich_station("u3", "C", "US", "rock,country,folk", "OGG", 128),
            ])
            .unwrap();
        // the limit only bounds tags; every country and codec is listed so a
        // smaller country is never hidden from the filter.
        let f = cache.facets(2).unwrap();
        assert_eq!(
            f.countries.len(),
            3,
            "all 3 countries listed regardless of limit"
        );
        assert_eq!(f.codecs.len(), 3, "all 3 codecs listed regardless of limit");
        assert_eq!(f.tags.len(), 2, "tags bounded by the limit");
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
        let rows = cache.search(&q, &[]).unwrap();
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
        let rows = cache.search(&q, &[]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stationuuid, "u1");
    }

    #[test]
    fn a_limited_search_returns_at_most_the_limit() {
        // "radio" matches 26,810 rows in the real cache; building all of them
        // cost seconds, so the cut has to happen in sqlite, not afterwards.
        let cache = Cache::open_in_memory().unwrap();
        let many: Vec<Station> = (0..500)
            .map(|i| {
                rich_station(
                    &format!("u{i}"),
                    &format!("Radio {i}"),
                    "PL",
                    "",
                    "MP3",
                    128,
                )
            })
            .collect();
        cache.upsert(&many).unwrap();
        let q = SearchQuery {
            name: Some("radio".into()),
            ..Default::default()
        };
        assert_eq!(cache.search_limited(&q, &[], Some(50)).unwrap().len(), 50);
        // no limit still means no limit, so every existing caller is unchanged.
        assert_eq!(cache.search(&q, &[]).unwrap().len(), 500);
    }

    #[test]
    fn a_limited_search_still_fills_its_page_past_duplicates() {
        // dedup runs after the fetch and can only drop rows; without headroom a
        // run of duplicates at the head of the list would return a short page.
        let cache = Cache::open_in_memory().unwrap();
        let mut many: Vec<Station> = (0..40)
            .map(|i| rich_station(&format!("dup{i}"), "Radio Same", "PL", "", "MP3", 128))
            .collect();
        many.extend((0..40).map(|i| {
            rich_station(
                &format!("u{i}"),
                &format!("Radio Unique {i}"),
                "PL",
                "",
                "MP3",
                128,
            )
        }));
        cache.upsert(&many).unwrap();
        let q = SearchQuery {
            name: Some("radio".into()),
            ..Default::default()
        };
        // 40 identical rows collapse to one, so a naive LIMIT 10 would answer 1.
        assert_eq!(cache.search_limited(&q, &[], Some(10)).unwrap().len(), 10);
    }

    #[test]
    fn a_limited_search_keeps_the_country_filter() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "Radio One", "PL", "", "MP3", 128),
                rich_station("u2", "Radio Two", "UA", "", "MP3", 128),
            ])
            .unwrap();
        let q = SearchQuery {
            countrycodes: vec!["UA".into()],
            ..Default::default()
        };
        let rows = cache.search_limited(&q, &[], Some(50)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stationuuid, "u2");
    }

    #[test]
    fn country_counts_matches_the_full_facet_call() {
        // the cheap path must answer the same countries as the expensive one —
        // it only skips the codec and tag work a country tree never shows.
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                rich_station("u1", "A", "PL", "pop", "MP3", 128),
                rich_station("u2", "B", "PL", "rock", "MP3", 128),
                rich_station("u3", "C", "UA", "jazz", "AAC", 64),
            ])
            .unwrap();
        assert_eq!(
            cache.country_counts().unwrap(),
            cache.facets(10).unwrap().countries
        );
        assert_eq!(
            cache.country_counts().unwrap(),
            vec![("PL".to_string(), 2), ("UA".to_string(), 1)]
        );
    }
}
