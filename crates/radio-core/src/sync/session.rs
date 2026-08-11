use super::{HistoryRecord, Lww, Pending, Profile, ProfileChange, SyncData};
use crate::catalog::favorites::HISTORY_CAP;
use crate::catalog::Play;

/// everything a surface holds locally at the moment it syncs. the three id
/// lists are all `Vec<String>`, so they are named rather than positional.
pub struct LocalState<'a> {
    pub favs: Vec<String>,
    pub blocked: Vec<String>,
    pub excluded_countries: Vec<String>,
    pub changed: Pending,
    pub profile: &'a Profile,
    pub plays: &'a [Play],
}

/// the one place an outgoing payload is assembled. every surface (tui worker,
/// `sync run`/`sync use`, the macos app) goes through it, so the omission
/// guards and the profile shapes can never drift between them.
pub fn outgoing(local: LocalState) -> SyncData {
    let payload = profile_payload(local.profile, local.plays);
    SyncData {
        favs: local.favs,
        blocked: local.blocked,
        excluded_countries: local.excluded_countries,
        changed: local.changed,
        shuffle_filter: payload.shuffle_filter,
        scope: payload.scope,
        theme: payload.theme,
        history: payload.history,
    }
}

/// builds the profile half of an outgoing payload. `_at == 0` means the field
/// was never touched on this device, and it is omitted entirely so the request
/// stays byte-identical to the pre-profile format.
pub fn profile_lww_filter(profile: &Profile) -> Option<Lww> {
    if profile.countries_at == 0 {
        return None;
    }
    Some(Lww {
        value: serde_json::json!({ "countries": profile.countries }),
        at: profile.countries_at,
    })
}

pub fn profile_lww_string(value: &str, at: i64) -> Option<Lww> {
    if at == 0 {
        return None;
    }
    Some(Lww {
        value: serde_json::Value::String(value.to_string()),
        at,
    })
}

pub fn remote_lww_filter(lww: &Option<Lww>) -> Option<(Vec<String>, i64)> {
    let lww = lww.as_ref()?;
    let countries = lww.value.get("countries")?.as_array()?;
    let countries = countries
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    Some((countries, lww.at))
}

pub fn remote_lww_string(lww: &Option<Lww>) -> Option<(String, i64)> {
    let lww = lww.as_ref()?;
    let value = lww.value.as_str()?.to_string();
    Some((value, lww.at))
}

/// each play carries the time it actually happened, captured when the station
/// was played and never rewritten — re-stamping here would make every local
/// entry outrank every remote one and pin them all at the top of the cap.
pub fn local_history_records(plays: &[Play]) -> Vec<HistoryRecord> {
    plays
        .iter()
        .map(|p| HistoryRecord {
            id: p.id.clone(),
            at: p.at,
            gone: false,
        })
        .collect()
}

/// the profile half of an outgoing payload, in one place so every surface
/// publishes the same shape.
pub struct ProfilePayload {
    pub shuffle_filter: Option<Lww>,
    pub scope: Option<Lww>,
    pub theme: Option<Lww>,
    pub history: Vec<HistoryRecord>,
}

pub fn profile_payload(profile: &Profile, plays: &[Play]) -> ProfilePayload {
    ProfilePayload {
        shuffle_filter: profile_lww_filter(profile),
        scope: profile_lww_string(&profile.scope, profile.scope_at),
        theme: profile_lww_string(&profile.theme, profile.theme_at),
        history: local_history_records(plays),
    }
}

/// takes whatever the server sent back that is newer than the local stamps.
/// the caller persists `profile` and pushes the reported changes into its ui.
pub fn apply_remote_profile(
    profile: &mut Profile,
    shuffle_filter: &Option<Lww>,
    scope: &Option<Lww>,
    theme: &Option<Lww>,
) -> ProfileChange {
    profile.apply_newer(
        remote_lww_filter(shuffle_filter),
        remote_lww_string(scope),
        remote_lww_string(theme),
    )
}

/// union by uuid, newest `at` wins, capped at HISTORY_CAP — the same rule the
/// server itself uses to merge two devices. returns `None` when there is
/// nothing remote to fold in, so the caller can skip the write.
pub fn merge_history(local: &[Play], remote: &[HistoryRecord]) -> Option<Vec<Play>> {
    if remote.is_empty() {
        return None;
    }
    let local = local_history_records(local);
    // a BTreeMap, not a HashMap: the sort below is stable, so the map's iteration
    // order decides ties on `at`. the server merges through a BTreeMap too, and
    // only an identical tie convention makes both sides truncate the same entry
    // at the cap.
    let mut by_id: std::collections::BTreeMap<&str, &HistoryRecord> =
        std::collections::BTreeMap::new();
    for r in local.iter().chain(remote.iter()) {
        match by_id.get(r.id.as_str()) {
            Some(have) if have.at >= r.at => {}
            _ => {
                by_id.insert(&r.id, r);
            }
        }
    }
    let mut merged: Vec<&HistoryRecord> = by_id.into_values().filter(|r| !r.gone).collect();
    merged.sort_by_key(|r| std::cmp::Reverse(r.at));
    merged.truncate(HISTORY_CAP);
    Some(
        merged
            .into_iter()
            .map(|r| Play {
                id: r.id.clone(),
                at: r.at,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local<'a>(profile: &'a Profile, plays: &'a [Play]) -> LocalState<'a> {
        LocalState {
            favs: vec!["f1".into()],
            blocked: vec![],
            excluded_countries: vec![],
            changed: Pending::default(),
            profile,
            plays,
        }
    }

    #[test]
    fn a_never_touched_device_sends_the_pre_profile_payload_verbatim() {
        // a pre-upgrade server must see byte-identical json to before.
        let p = Profile::default();
        let out = outgoing(local(&p, &[]));
        let j = serde_json::to_string(&out).unwrap();
        assert_eq!(
            j, r#"{"favs":["f1"],"blocked":[],"excluded_countries":[]}"#,
            "an untouched device must not grow any new keys"
        );
    }

    #[test]
    fn outgoing_carries_the_profile_once_it_is_stamped() {
        let mut p = Profile::default();
        p.set_scope("blocked", 7);
        let out = outgoing(local(
            &p,
            &[Play {
                id: "s1".into(),
                at: 9,
            }],
        ));
        assert_eq!(out.scope.unwrap().value, serde_json::json!("blocked"));
        assert_eq!(out.history.len(), 1);
        assert!(out.shuffle_filter.is_none());
        assert!(out.theme.is_none());
    }

    #[test]
    fn a_never_touched_profile_publishes_nothing() {
        let p = Profile::default();
        let payload = profile_payload(&p, &[]);
        assert!(payload.shuffle_filter.is_none());
        assert!(payload.scope.is_none());
        assert!(payload.theme.is_none());
        assert!(payload.history.is_empty());
    }

    #[test]
    fn a_touched_profile_publishes_the_agreed_shapes() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 10);
        p.set_scope("recent", 20);
        p.set_theme("nord", 30);
        let payload = profile_payload(
            &p,
            &[Play {
                id: "s1".into(),
                at: 40,
            }],
        );
        let f = payload.shuffle_filter.unwrap();
        assert_eq!(f.value, serde_json::json!({"countries": ["UA"]}));
        assert_eq!(f.at, 10);
        assert_eq!(payload.scope.unwrap().value, serde_json::json!("recent"));
        assert_eq!(payload.theme.unwrap().value, serde_json::json!("nord"));
        assert_eq!(payload.history[0].id, "s1");
        assert_eq!(payload.history[0].at, 40);
        assert!(!payload.history[0].gone);
    }

    #[test]
    fn an_empty_countries_list_still_publishes_once_stamped() {
        // clearing the filter is a real change; omitting it would let a stale
        // remote list come back on the next sync.
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 10);
        p.set_countries(vec![], 20);
        let payload = profile_payload(&p, &[]);
        assert_eq!(
            payload.shuffle_filter.unwrap().value,
            serde_json::json!({"countries": []})
        );
    }

    #[test]
    fn apply_remote_profile_takes_only_newer_fields() {
        let mut p = Profile::default();
        p.set_scope("all", 100);
        p.set_theme("amber-crt", 100);
        let changed = apply_remote_profile(
            &mut p,
            &None,
            &Some(Lww {
                value: serde_json::json!("dead"),
                at: 50,
            }),
            &Some(Lww {
                value: serde_json::json!("nord"),
                at: 200,
            }),
        );
        assert!(!changed.scope);
        assert!(changed.theme);
        assert_eq!(p.scope, "all");
        assert_eq!(p.theme, "nord");
    }

    #[test]
    fn apply_remote_profile_reads_the_filter_wrapper() {
        let mut p = Profile::default();
        let changed = apply_remote_profile(
            &mut p,
            &Some(Lww {
                value: serde_json::json!({"countries": ["UA", "PL"]}),
                at: 5,
            }),
            &None,
            &None,
        );
        assert!(changed.countries);
        assert_eq!(p.countries, vec!["UA".to_string(), "PL".to_string()]);
    }

    #[test]
    fn apply_remote_profile_ignores_a_malformed_filter() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 1);
        let changed = apply_remote_profile(
            &mut p,
            &Some(Lww {
                value: serde_json::json!("not-an-object"),
                at: 99,
            }),
            &None,
            &None,
        );
        assert!(!changed.any());
        assert_eq!(p.countries, vec!["UA".to_string()]);
    }

    #[test]
    fn local_history_records_are_stable_across_two_syncs() {
        // the defect: re-stamping every local id at sync time made all of them
        // outrank every remote play and pin them at the top of the cap, so a
        // station played on another device could never reach this one.
        let plays = vec![
            Play {
                id: "u1".into(),
                at: 1_700_000_000,
            },
            Play {
                id: "u2".into(),
                at: 1_600_000_000,
            },
        ];
        let once = local_history_records(&plays);
        assert_eq!(once[0].at, 1_700_000_000);
        assert_eq!(once[1].at, 1_600_000_000);
        assert_eq!(once, local_history_records(&plays), "must not re-stamp");
    }

    #[test]
    fn merge_history_with_nothing_remote_is_none() {
        assert!(merge_history(
            &[Play {
                id: "u1".into(),
                at: 1
            }],
            &[]
        )
        .is_none());
    }

    #[test]
    fn a_newer_remote_play_outranks_an_older_local_one() {
        let merged = merge_history(
            &[Play {
                id: "local".into(),
                at: 1_000,
            }],
            &[HistoryRecord {
                id: "remote".into(),
                at: 2_000,
                gone: false,
            }],
        )
        .unwrap();
        assert_eq!(merged[0].id, "remote");
        assert_eq!(merged[1].id, "local");
        assert_eq!(merged[1].at, 1_000);
    }

    // the server merges through a BTreeMap and then stable-sorts by `at`, so on a
    // tie it emits the ids in ascending order. the client must land on the same
    // order or the two truncate a different entry at the cap.
    #[test]
    fn ties_on_at_order_by_id_exactly_as_the_server_does() {
        let remote: Vec<HistoryRecord> = ["u3", "u1", "u4", "u2"]
            .iter()
            .map(|id| HistoryRecord {
                id: (*id).into(),
                at: 500,
                gone: false,
            })
            .collect();
        for _ in 0..20 {
            let merged = merge_history(&[], &remote).unwrap();
            let ids: Vec<&str> = merged.iter().map(|p| p.id.as_str()).collect();
            assert_eq!(ids, ["u1", "u2", "u3", "u4"]);
        }
    }

    #[test]
    fn merge_history_drops_tombstones_and_caps_at_two_hundred() {
        let remote: Vec<HistoryRecord> = (0..300)
            .map(|i| HistoryRecord {
                id: format!("u{i}"),
                at: 1_000_000 - i,
                gone: i == 0,
            })
            .collect();
        let merged = merge_history(&[], &remote).unwrap();
        assert_eq!(merged.len(), HISTORY_CAP);
        assert!(merged.iter().all(|p| p.id != "u0"));
    }
}
