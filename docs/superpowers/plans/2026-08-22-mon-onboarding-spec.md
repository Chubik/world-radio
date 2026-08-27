# world-radio → mon: monitoring design

**Status:** agreed 2026-08-22
**Source contract:** `mon/docs/ONBOARDING.md` (read in full; this document does
not restate it, it records the decisions this project makes against it)
**Target level:** 2 (logs + health + full metrics), no telegram notices

---

## Why this exists

The previous monitoring was hand-rolled and does not match the mon contract.
`mon` already scrapes us — `prometheus/prometheus.yml` has a `world-radio`
blackbox target and two scrape jobs (`world-radio-stat` on 8137,
`world-radio-sync` on 8138) — but the metrics those endpoints serve carry names
nothing in mon queries. `mon/dashboards/project.json` is provisioned and asks
for `up`, `probe_success`, `build_info`, `http_requests_total`,
`http_request_duration_seconds_bucket`, `job_last_success_timestamp_seconds`,
`job_errors_total` and Loki `{project="world-radio"}` with a `level` label. We
serve none of those, so the world-radio dashboard is nearly empty today.

So this is not "add monitoring". It is: **delete the old surface, emit the
contract instead.**

## Decisions taken

| question | decision |
|---|---|
| old `/metrics` in sync and stat | **removed entirely**, including `stat → sync` HTTP scraping (`sync_stats.rs`) |
| the `/stat` admin page | **removed entirely** — Grafana replaces it; `render.rs` goes |
| "Recent downloads" (per-event rows) | to **Loki**, by having `stat` re-emit new download events as JSON lines on **stdout**; Alloy collects stdout automatically, so `mon` needs no change for this |
| adoption level | **2** — JSON logs, `LOG_LEVEL`, `/healthz`, contract metrics |
| telegram notices via mon webhook | **not now** — stat already has its own direct telegram notifier and it stays |
| compose layout | **three separate compose files, each pinned `name: world-radio`** — no merge |

### Why the compose files are not merged

`ops/deploy/docker-compose.prod.yml` already has `name: world-radio`. `sync/` and
`stat/` have no `name:` at all, so today they land in Loki as projects `sync`
and `stat`.

They must **not** be merged into one file. Each repo deploys itself from its own
workflow, and `ops/.github/workflows/deploy.yml` carries an explicit comment
recording that deploying sync from ops caused colliding container recreations on
2026-08-14 ("do not add a sync deploy step back here"). Loki takes the label
from `com.docker.compose.project`, not from file layout — three files sharing
one `name:` give one project in Grafana with no deploy change at all.

Volumes: `ops` already pins `world-radio_releases`. Adding `name: world-radio`
to `sync/` and `stat/` **renames their volumes** (`sync_sync-data` →
`world-radio_sync-data`) and would strand the sqlite database and the notify
mark. Both volumes must be pinned to their current names in the same edit.
Their current names must be read off the host before the edit, not guessed.

## Ports

The contract requires `/metrics` on its **own** listener, published on
`127.0.0.1` only, never through nginx. Today `/metrics` sits on the public mux
on 8137/8138, which are also published on `172.17.0.1` (the docker bridge) so
stat could reach sync.

Taken from the contract's list of ports in use: 8080 subtick, 8091 cybdash,
8137 world-radio stat, 8138 world-radio sync, 9091 diffalarm.

| service | public (unchanged) | new metrics listener |
|---|---|---|
| stat | 8137 | **8147** |
| sync | 8138 | **8148** |

The metrics ports are published `127.0.0.1` only. The existing `172.17.0.1`
publishes exist solely for the stat→sync scrape, which this work deletes; they
go with it.

`mon/prometheus/prometheus.yml` must be repointed 8137→8147 and 8138→8148. That
is one PR against `mon`, and it is the only change mon needs.

## Metrics contract

Unprefixed contract metrics on both services:

- `http_requests_total{route,method,status}` — `route` is the **axum route
  pattern** (`/catalog/delta`), never the raw path; unmatched → `"unmatched"`
- `http_request_duration_seconds{route,method}` — buckets
  `.005 .01 .025 .05 .1 .25 .5 1 2.5 5 10`, **no** `status` label
- `job_last_success_timestamp_seconds{job}` — set only after a successful run,
  never pre-set at startup
- `job_interval_seconds{job}` — set once at registration
- `job_errors_total{job}` — one increment per failed run
- `build_info{version}` — always 1, set once from `main`

Jobs that exist and must be instrumented:

| service | job name | interval | today |
|---|---|---|---|
| sync | `catalog` | `REFRESH_SECS` | `catalog::spawn_refresher`, already runs once before sleeping |
| stat | `github-poll` | 900 s | the poll loop in `stat/src/main.rs`, already runs once before sleeping |

Both already run once at startup, which the silent-worker alert requires
(it tolerates only `2 × interval + 10 min` of a zero gauge).

Business metrics keep a project prefix and live on the same endpoint. Carried
over from the old surface because the dashboard/KPIs need them:

- sync: `sync_accounts_total`, `sync_favs_total`, `sync_play_events_total`,
  `sync_active_streams`, `sync_last_activity_seconds`,
  `sync_catalog_stations`, `sync_catalog_bytes`, `sync_catalog_refreshed_seconds`
- stat: `downloads_total{country,version}`, `github_apk_downloads_total{version}`,
  plus new `github_cli_downloads_total{version}` and
  `github_app_downloads_total{version}` — the `/stat` page showed cli and app
  columns that had no metric, and removing the page must not lose them

`country` stays a label: ~200 enumerable values, which satisfies the
contract's cardinality rule. IP and user agent are **never** labels and never
logged.

## Logs

JSON to stdout, `time` / `level` / `msg` / `service`, `LOG_LEVEL` honoured with
an `INFO` default and one `WARN` on an unknown value. Rust means `tracing` +
`tracing-subscriber` json, not the doc's Go `obs` package — the contract cares
about the wire format, not the library.

Every current `println!`/`eprintln!` in both services becomes a `tracing` call
with a constant `msg` and values as fields.

Access line: one per request, `method` `path` `status` `duration_ms`, at `ERROR`
when status ≥ 500, `INFO` otherwise. `X-Request-Id` accepted from the proxy when
well-formed (`^[A-Za-z0-9_-]{8,64}$`), otherwise minted, always echoed, and
attached as `request_id` to every request-scoped line.

Panics are recovered and logged at `ERROR` with a stack.

### Download events to Loki

`stat` already tails `/logs/releases.log` every 900 s and already tracks a
persisted mark (`notify::advance_mark`) so it only announces *new* entries. The
same freshly-seen events get one `INFO` line each on stdout:

```
msg="download", file, version, country
```

No `ip`, no `ua` — the contract forbids both. Alloy picks stdout up with no mon
change, and `{project="world-radio", service="stat"} | json | msg="download"`
reproduces the old Recent-downloads table with filtering it never had.

## Health

`GET /healthz` on both, 200 only when hard dependencies answer within 2 s:

- sync: sqlite `SELECT 1`
- stat: the release log is readable

Body `{"status","checks","version"}`, 503 when degraded, error text to the log
not the body. The existing `/health` stays as an alias — the deploy workflow
polls it.

Compose gets a healthcheck hitting `/healthz`.

## What is deleted

- `sync/src/main.rs`: the `metrics` handler's hand-formatted string
- `stat/src/main.rs`: the `/stat` route, the `metrics` handler's old body
- `stat/src/render.rs` — whole file
- `stat/src/sync_stats.rs` — whole file (stat no longer scrapes sync)
- `stat/src/aggregate.rs`: `render_metrics` / `render_apk_metrics` string
  builders, replaced by registry gauges
- `AppState.sync` in stat and the `SYNC_METRICS_URL` env
- the `172.17.0.1` port publishes in both compose files

`stat`'s telegram notifier, github polling, `/android-info` and `/cli-info` all
stay — the site reads the last two.

## Out of scope

- the telegram webhook in mon (stat notifies directly already)
- any alert rule authoring in mon (the provisioned alerts key off contract
  metric names, which is exactly what this work starts emitting)
- the CLI, macOS and Android clients — they are not services

## Done means

- `curl -s 127.0.0.1:8147/metrics | grep -c http_requests_total` is non-zero on
  the host, same for 8148
- the world-radio dashboard in Grafana shows populated health, http, logs and
  worker rows
- `{project="world-radio"}` in Loki shows both `stat` and `sync`, with a `level`
  label
- one PR against `mon` repointing the two scrape jobs

---

## Host facts, read 2026-08-22

Host: `appshost` — its address, ssh port and user are in `mon/docs/RUNBOOK.md`
and in the private infra notes. They are deliberately not repeated here: this
repository is public, and an address plus a non-default port plus a username is
most of what a scanner needs.

`docker compose ls` — world-radio is **three** projects today, not one:

```
world-radio         running(1)   /opt/world-radio/deploy/docker-compose.prod.yml
world-radio-stat    running(1)   /opt/world-radio-stat/docker-compose.yml
world-radio-sync    running(1)   /opt/world-radio-sync/docker-compose.yml
```

The two service compose files have no `name:`, so docker derived the project
name from their **directory** — `/opt/world-radio-sync` → `world-radio-sync`.
That is why the plan's guess of `sync`/`stat` was wrong.

Volumes actually in use, from `docker inspect` on the running containers:

| container | volume name | mounted at |
|---|---|---|
| `world-radio-sync` | **`world-radio-sync_sync-data`** | `/data` |
| `world-radio-stat` | **`world-radio-stat_stat-data`** | `/data` |

`docker volume ls` also shows a stray `sync-data` that no running container
mounts — an orphan from an earlier layout. Leave it alone; deleting it is not
part of this work.

`world-radio_releases` is the ops volume and is already pinned in
`ops/deploy/docker-compose.prod.yml`.

**Consequence for Task 10.** Setting `name: world-radio` in the sync and stat
compose files changes their project name from `world-radio-sync` /
`world-radio-stat` to `world-radio`, which renames their volumes to
`world-radio_sync-data` / `world-radio_stat-data` and strands the accounts
database and the notify mark. Both volumes must be pinned to the names in the
table above.
