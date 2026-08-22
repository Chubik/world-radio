# world-radio → mon monitoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the hand-rolled monitoring in `sync` and `stat` and emit the `mon` contract instead — JSON logs, `/healthz`, and contract metrics on a private listener — so the already-provisioned Grafana `world-radio` dashboard populates.

**Architecture:** Both services are Rust/axum. Each grows a small `obs` module (the Rust equivalent of the contract's Go `obs` package): `tracing`-json logging, a request-id + access-log + metrics middleware stack, a `prometheus` registry, and a `run_job` helper. Contract metrics move to a **second axum server** on a loopback-only port (stat 8147, sync 8148); the public mux keeps only product routes plus `/healthz`. The `/stat` HTML admin page and stat's HTTP scrape of sync are deleted — Grafana replaces both, and per-download events reach Grafana through Loki as stdout JSON lines.

**Tech Stack:** Rust 2021, axum 0.7, tokio, `tracing` + `tracing-subscriber` (json), `prometheus` crate (registry + text encoder), docker compose, Prometheus/Loki/Grafana in the `mon` stack.

**Spec:** `docs/superpowers/plans/2026-08-22-mon-onboarding-spec.md`

## Global Constraints

- **Repos are separate.** `radio`, `ops`, `sync`, `stat` are four independent git repos under `world-radio/`. `sync/` and `stat/` changes commit and push from those directories. Never assume a change in one reaches another.
- **Branch:** all work commits to `dev` in each repo. Never commit to `main`.
- **Commit messages:** English, meaningful and concise, no AI/assistant mention, no `Co-Authored-By`, no "Generated with" trailer. The subject line is a public changelog entry.
- **Log and comment text:** English, lower case, no capital first letter.
- **The project name is exactly `world-radio`** — lower case, hyphen — in every compose `name:`, every `project:` label, everywhere. No variants.
- **Metrics ports:** stat `8147`, sync `8148`. Published `127.0.0.1` only, never `0.0.0.0`, never through nginx. In use elsewhere on the host and unavailable: 8080, 8091, 8132, 8137, 8138, 9091.
- **Contract metric names are unprefixed and exact:** `http_requests_total{route,method,status}`, `http_request_duration_seconds{route,method}`, `job_last_success_timestamp_seconds{job}`, `job_interval_seconds{job}`, `job_errors_total{job}`, `build_info{version}`. Business metrics keep a `sync_`/project prefix.
- **Histogram buckets, exactly:** `0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0`. No `status` label on the histogram.
- **Never a label or a log field:** ip, user agent, raw request path, user id, e-mail, query string, tokens, keys.
- **Rust style:** `cargo fmt` before every commit; `cargo clippy` clean. Follow the surrounding file's style — these files use `match` over `if let` chains in places and lower-case comments explaining *why*.
- **Tests:** `cargo test` in the touched repo must pass before any commit. `cargo test` takes **one** filter argument, not two.

---

## File Structure

**`sync/` repo**

| file | responsibility |
|---|---|
| `src/obs.rs` (create) | logger init, request id, access log, metrics middleware, registry, `run_job`, `build_info`, metrics text rendering |
| `src/main.rs` (modify) | wire obs in, add `/healthz`, drop the old `metrics` handler, spawn the metrics server |
| `src/catalog.rs` (modify) | refresher reports job success/failure through obs; `println!` → `tracing` |
| `src/store.rs` (modify) | add `ping()` for `/healthz` |
| `src/mirror.rs`, `src/merge.rs`, `src/key.rs` (modify) | `println!` → `tracing` only if any exist |
| `Cargo.toml` (modify) | add `tracing`, `tracing-subscriber`, `prometheus` |
| `docker-compose.yml` (modify) | `name: world-radio`, pinned volume, metrics port, `LOG_LEVEL`, healthcheck, drop `172.17.0.1` publish |

**`stat/` repo**

| file | responsibility |
|---|---|
| `src/obs.rs` (create) | same as sync's — the two copies are deliberate, the repos are independent |
| `src/main.rs` (modify) | drop `/stat` and the old `metrics`, add `/healthz`, spawn the metrics server, emit download log lines, feed business gauges |
| `src/aggregate.rs` (modify) | string builders → registry gauge updates |
| `src/render.rs` (delete) | the HTML admin page is gone |
| `src/sync_stats.rs` (delete) | stat no longer scrapes sync |
| `src/notify.rs`, `src/github.rs`, `src/store.rs` (modify) | `println!`/`eprintln!` → `tracing` |
| `Cargo.toml` (modify) | add `tracing`, `tracing-subscriber`, `prometheus` |
| `docker-compose.yml` (modify) | as sync's |

**`ops/` repo**

| file | responsibility |
|---|---|
| `deploy/docker-compose.prod.yml` (modify) | nothing structural — verify `name: world-radio` is present (it is) and that the site does not link `/stat` |
| `site/**` (modify if needed) | remove any link to the removed `/stat` page |

**`mon/` repo (one PR, last)**

| file | responsibility |
|---|---|
| `prometheus/prometheus.yml` (modify) | repoint `world-radio-stat` 8137→8147 and `world-radio-sync` 8138→8148 |

---

## Task 1: Read the host before changing any compose file

This task changes no code. It exists because two later tasks can strand production data if their inputs are guessed, and both inputs live only on the host.

**Files:** none — this task records findings into the plan's own notes.

**Interfaces:**
- Produces: the exact current docker volume names for sync and stat, and the current `docker compose ls` project names. Tasks 8 and 9 pin these verbatim.

- [ ] **Step 1: List the compose projects on the host**

The host is reached over SSH; use the existing deploy access.

Run on the host:
```bash
docker compose ls
```

Record the exact output. Expect three entries today, likely `world-radio` (from ops), `sync` and `stat`.

- [ ] **Step 2: List the volumes for sync and stat**

Run on the host:
```bash
docker volume ls | grep -Ei 'sync|stat|world-radio'
```

Record the exact names. Expect something like `sync_sync-data` and `stat_stat-data`.

- [ ] **Step 3: Confirm which volume each container actually uses**

Run on the host:
```bash
docker inspect world-radio-sync --format '{{json .Mounts}}'
docker inspect world-radio-stat --format '{{json .Mounts}}'
```

The `Name` field of the non-bind mount is the string that must be pinned. Do not infer it from the compose file — infer it from the running container.

- [ ] **Step 4: Write the findings into the spec**

Append a short "host facts, read 2026-08-22" section to
`docs/superpowers/plans/2026-08-22-mon-onboarding-spec.md` recording the three
outputs verbatim.

- [ ] **Step 5: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add docs/superpowers/plans/2026-08-22-mon-onboarding-spec.md
git commit -m "record the host volume names monitoring must preserve"
```

---

## Task 2: The `obs` module in sync — logging

**Files:**
- Create: `sync/src/obs.rs`
- Modify: `sync/Cargo.toml`
- Modify: `sync/src/main.rs:1-5` (add `mod obs;`)

**Interfaces:**
- Produces:
  - `obs::init_logging(service: &str)` — installs the global json subscriber, returns nothing
  - `obs::level_from_env() -> (tracing::Level, Option<String>)` — parsed level plus the raw bad value when it fell back

- [ ] **Step 1: Add the dependencies**

In `sync/Cargo.toml`, under `[dependencies]`:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
prometheus = { version = "0.14", default-features = false }
```

`prometheus` is added now so later tasks in this repo do not touch `Cargo.toml` again. `default-features = false` drops its optional protobuf and process-collector extras — the contract only needs the text format.

- [ ] **Step 2: Write the failing test**

Create `sync/src/obs.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_levels_parse_in_any_case() {
        assert_eq!(parse_level("debug"), (tracing::Level::DEBUG, None));
        assert_eq!(parse_level("WARN"), (tracing::Level::WARN, None));
        assert_eq!(parse_level("Error"), (tracing::Level::ERROR, None));
    }

    #[test]
    fn blank_is_info_without_a_complaint() {
        assert_eq!(parse_level(""), (tracing::Level::INFO, None));
        assert_eq!(parse_level("   "), (tracing::Level::INFO, None));
    }

    #[test]
    fn unknown_falls_back_to_info_and_reports_the_value() {
        assert_eq!(
            parse_level("verbose"),
            (tracing::Level::INFO, Some("verbose".to_string()))
        );
    }
}
```

Add `mod obs;` to the top of `sync/src/main.rs`, next to the other `mod` lines.

- [ ] **Step 3: Run the test to verify it fails**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test parse_level
```

Expected: compile error — `cannot find function 'parse_level'`.

- [ ] **Step 4: Implement**

Prepend to `sync/src/obs.rs`:

```rust
use tracing::Level;
use tracing_subscriber::fmt::time::UtcTime;

/// reads LOG_LEVEL. an unknown value is not fatal: we fall back to info and
/// return the offending string so the caller can log one warning about it.
fn parse_level(raw: &str) -> (Level, Option<String>) {
    let raw = raw.trim();
    if raw.is_empty() {
        return (Level::INFO, None);
    }
    match raw.to_ascii_lowercase().as_str() {
        "trace" => (Level::TRACE, None),
        "debug" => (Level::DEBUG, None),
        "info" => (Level::INFO, None),
        "warn" | "warning" => (Level::WARN, None),
        "error" => (Level::ERROR, None),
        _ => (Level::INFO, Some(raw.to_string())),
    }
}

/// installs the process-wide json logger. service is this binary's role inside
/// the project: "sync", "stat".
pub fn init_logging(service: &str) {
    let raw = std::env::var("LOG_LEVEL").unwrap_or_default();
    let (level, bad) = parse_level(&raw);
    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_max_level(level)
        .with_timer(UtcTime::rfc_3339())
        .with_current_span(false)
        .with_span_list(false)
        .init();
    // every line carries the service; tracing has no global "with", so the
    // field is attached by the helpers below rather than by the subscriber.
    SERVICE
        .set(service.to_string())
        .expect("init_logging called twice");
    if let Some(value) = bad {
        tracing::warn!(service = %service, value = %value, "unknown LOG_LEVEL, using INFO");
    }
}

pub static SERVICE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// the service name for log fields; "unknown" before init_logging runs.
pub fn service() -> &'static str {
    SERVICE.get().map(String::as_str).unwrap_or("unknown")
}
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test parse_level
```

Expected: 3 passed.

- [ ] **Step 6: Verify the wire format by eye**

The contract requires `time`, `level`, `msg`, `service` keys. `tracing-subscriber`'s json emits `timestamp`, `level`, `message` (plus fields, flattened). The ONBOARDING doc's Rust section states this explicitly and accepts it: *"This emits `timestamp` / `level` / `message` rather than `time` / `level` / `msg`. The collector only needs `level`, so it is fine as is."*

No renaming work is needed. Record this in a comment above `init_logging`:

```rust
/// the collector only promotes `level` to a loki label; `timestamp`/`message`
/// are queried under those names, which the mon contract explicitly allows for
/// rust services.
```

- [ ] **Step 7: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo fmt
cargo clippy --all-targets
cargo test
git add Cargo.toml Cargo.lock src/obs.rs src/main.rs
git commit -m "log as json, with the level set by LOG_LEVEL"
```

---

## Task 3: The `obs` module in sync — metrics registry and middleware

**Files:**
- Modify: `sync/src/obs.rs`
- Test: inline `#[cfg(test)] mod tests` in `sync/src/obs.rs`

**Interfaces:**
- Consumes: `obs::service()` from Task 2
- Produces:
  - `obs::registry() -> &'static prometheus::Registry`
  - `obs::render_metrics() -> String` — the text exposition of everything registered
  - `obs::set_build_info(version: &str)`
  - `obs::observe_request(route: &str, method: &str, status: u16, secs: f64)`
  - `obs::track(req, next)` — an axum middleware fn recording the two http metrics and writing the access line
  - `obs::request_id_layer()` — see Task 4; not in this task

- [ ] **Step 1: Write the failing test**

Append to `sync/src/obs.rs`'s test module:

```rust
    #[test]
    fn renders_the_contract_http_metrics() {
        observe_request("/catalog", "GET", 200, 0.012);
        let out = render_metrics();
        assert!(out.contains(r#"http_requests_total{method="GET",route="/catalog",status="200"} 1"#));
        assert!(out.contains("http_request_duration_seconds_bucket"));
        assert!(out.contains(r#"le="0.025""#));
    }

    #[test]
    fn build_info_is_one_and_carries_the_version() {
        set_build_info("1.22.4");
        let out = render_metrics();
        assert!(out.contains(r#"build_info{version="1.22.4"} 1"#));
    }

    #[test]
    fn the_histogram_has_no_status_label() {
        observe_request("/catalog", "GET", 500, 0.4);
        let out = render_metrics();
        for line in out.lines().filter(|l| l.starts_with("http_request_duration_seconds")) {
            assert!(!line.contains("status="), "histogram must not carry status: {line}");
        }
    }
```

These tests share one process-global registry, so they must not assert on exact
counter totals beyond the first — that is why only the first asserts `1` and it
uses a route no other test touches.

- [ ] **Step 2: Run to verify it fails**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test obs::tests
```

Expected: compile error — `cannot find function 'observe_request'`.

- [ ] **Step 3: Implement the registry**

Append to `sync/src/obs.rs`:

```rust
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};
use std::sync::OnceLock;

/// the buckets are fixed by the mon contract; every project uses the same ones
/// so one dashboard serves them all.
const BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

pub struct Metrics {
    pub registry: Registry,
    pub http_requests: IntCounterVec,
    pub http_duration: HistogramVec,
    pub job_last_success: IntGaugeVec,
    pub job_interval: IntGaugeVec,
    pub job_errors: IntCounterVec,
    pub build_info: IntGaugeVec,
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

pub fn metrics() -> &'static Metrics {
    METRICS.get_or_init(|| {
        let registry = Registry::new();
        let http_requests = IntCounterVec::new(
            Opts::new("http_requests_total", "requests by route pattern, method and status"),
            &["route", "method", "status"],
        )
        .expect("http_requests_total");
        let http_duration = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "request duration by route pattern and method",
            )
            .buckets(BUCKETS.to_vec()),
            &["route", "method"],
        )
        .expect("http_request_duration_seconds");
        let job_last_success = IntGaugeVec::new(
            Opts::new(
                "job_last_success_timestamp_seconds",
                "unix time of the last successful run",
            ),
            &["job"],
        )
        .expect("job_last_success_timestamp_seconds");
        let job_interval = IntGaugeVec::new(
            Opts::new("job_interval_seconds", "expected seconds between runs"),
            &["job"],
        )
        .expect("job_interval_seconds");
        let job_errors = IntCounterVec::new(
            Opts::new("job_errors_total", "failed runs"),
            &["job"],
        )
        .expect("job_errors_total");
        let build_info = IntGaugeVec::new(
            Opts::new("build_info", "always 1, carries the version"),
            &["version"],
        )
        .expect("build_info");
        for c in [
            Box::new(http_requests.clone()) as Box<dyn prometheus::core::Collector>,
            Box::new(http_duration.clone()),
            Box::new(job_last_success.clone()),
            Box::new(job_interval.clone()),
            Box::new(job_errors.clone()),
            Box::new(build_info.clone()),
        ] {
            registry.register(c).expect("register");
        }
        Metrics {
            registry,
            http_requests,
            http_duration,
            job_last_success,
            job_interval,
            job_errors,
            build_info,
        }
    })
}

pub fn registry() -> &'static Registry {
    &metrics().registry
}

pub fn render_metrics() -> String {
    let mut buf = Vec::new();
    let encoder = TextEncoder::new();
    if let Err(e) = encoder.encode(&registry().gather(), &mut buf) {
        tracing::error!(service = %service(), err = %e, "metrics encode failed");
        return String::new();
    }
    String::from_utf8(buf).unwrap_or_default()
}

pub fn set_build_info(version: &str) {
    metrics().build_info.with_label_values(&[version]).set(1);
}

pub fn observe_request(route: &str, method: &str, status: u16, secs: f64) {
    let m = metrics();
    m.http_requests
        .with_label_values(&[route, method, &status.to_string()])
        .inc();
    m.http_duration
        .with_label_values(&[route, method])
        .observe(secs);
}
```

- [ ] **Step 4: Run to verify it passes**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test obs::tests
```

Expected: all pass.

- [ ] **Step 5: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo fmt
cargo clippy --all-targets
cargo test
git add src/obs.rs
git commit -m "expose the standard request and build metrics"
```

---

## Task 4: The access log and request-id middleware in sync

**Files:**
- Modify: `sync/src/obs.rs`
- Test: inline

**Interfaces:**
- Consumes: `observe_request`, `service()`
- Produces:
  - `obs::safe_request_id(incoming: Option<&str>) -> Option<String>` — validates a proxy-supplied id
  - `obs::track(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response` — the single middleware doing request-id, metrics and the access line
  - `obs::route_of(req: &axum::extract::Request) -> String` — the matched axum route pattern, `"unmatched"` when there is none

- [ ] **Step 1: Write the failing test**

Append to the test module in `sync/src/obs.rs`:

```rust
    #[test]
    fn a_well_formed_incoming_id_is_kept() {
        assert_eq!(
            safe_request_id(Some("a3f1c9d2b4e5")),
            Some("a3f1c9d2b4e5".to_string())
        );
    }

    #[test]
    fn a_malformed_incoming_id_is_rejected() {
        assert_eq!(safe_request_id(Some("short")), None);
        assert_eq!(safe_request_id(Some("has spaces in it")), None);
        assert_eq!(safe_request_id(Some(&"x".repeat(65))), None);
        assert_eq!(safe_request_id(None), None);
    }

    #[test]
    fn a_minted_id_is_accepted_by_our_own_validator() {
        let id = mint_request_id();
        assert_eq!(safe_request_id(Some(&id)), Some(id));
    }
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test safe_request_id
```

Expected: compile error — `cannot find function 'safe_request_id'`.

- [ ] **Step 3: Implement**

Append to `sync/src/obs.rs`:

```rust
use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

/// accepts an id from a trusted proxy only when it is plainly safe to echo and
/// to put in a log field: a bounded, alphanumeric token.
pub fn safe_request_id(incoming: Option<&str>) -> Option<String> {
    let id = incoming?.trim();
    let ok = (8..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    match ok {
        true => Some(id.to_string()),
        false => None,
    }
}

pub fn mint_request_id() -> String {
    use rand::Rng;
    let bytes: [u8; 8] = rand::thread_rng().gen();
    hex::encode(bytes)
}

/// the matched route pattern, never the raw path — a path label would be
/// unbounded and would blow up cardinality.
pub fn route_of(req: &Request) -> String {
    match req.extensions().get::<MatchedPath>() {
        Some(p) => p.as_str().to_string(),
        None => "unmatched".to_string(),
    }
}

/// one middleware for the whole contract: request id in and out, the two http
/// metrics, and one access line per request.
pub async fn track(mut req: Request, next: Next) -> Response {
    let started = std::time::Instant::now();
    let method = req.method().to_string();
    let route = route_of(&req);
    let path = req.uri().path().to_string();

    let incoming = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok());
    let id = safe_request_id(incoming).unwrap_or_else(mint_request_id);
    if let Ok(value) = axum::http::HeaderValue::from_str(&id) {
        req.headers_mut().insert("x-request-id", value.clone());
        let mut res = next.run(req).await;
        res.headers_mut().insert("x-request-id", value);
        return finish(res, started, &method, &route, &path, &id);
    }
    let res = next.run(req).await;
    finish(res, started, &method, &route, &path, &id)
}

fn finish(
    res: Response,
    started: std::time::Instant,
    method: &str,
    route: &str,
    path: &str,
    id: &str,
) -> Response {
    let status = res.status().as_u16();
    let elapsed = started.elapsed();
    observe_request(route, method, status, elapsed.as_secs_f64());
    let duration_ms = elapsed.as_millis() as u64;
    // 4xx stays info on purpose: scanners and bad clients are not incidents.
    match status >= 500 {
        true => tracing::error!(
            service = %service(), request_id = %id, method = %method,
            path = %path, status = status, duration_ms = duration_ms,
            "http request"
        ),
        false => tracing::info!(
            service = %service(), request_id = %id, method = %method,
            path = %path, status = status, duration_ms = duration_ms,
            "http request"
        ),
    }
    res
}
```

Note `path` is a **log field**, not a metric label — the contract forbids it as
a label but the access line is where an operator needs the real path.

- [ ] **Step 4: Run to verify it passes**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test obs::tests
```

Expected: all pass.

- [ ] **Step 5: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo fmt
cargo clippy --all-targets
cargo test
git add src/obs.rs
git commit -m "trace every request with an id and one access line"
```

---

## Task 5: `run_job` and the sync catalogue refresher

**Files:**
- Modify: `sync/src/obs.rs`
- Modify: `sync/src/catalog.rs:475-490` (the refresher) and every `println!` in the file
- Test: inline in `obs.rs`

**Interfaces:**
- Consumes: `metrics()`
- Produces:
  - `obs::register_job(job: &str, interval_secs: u64)` — sets `job_interval_seconds`
  - `obs::job_succeeded(job: &str)` — sets `job_last_success_timestamp_seconds` to now
  - `obs::job_failed(job: &str)` — increments `job_errors_total`

Note the contract's rule: `job_last_success_timestamp_seconds` is **never**
pre-set at startup and **never** set on a failed run. `catalog::refresh` already
returns without changing what is served when a refresh fails, so the refresher
must learn whether it succeeded — see Step 3.

- [ ] **Step 1: Write the failing test**

Append to `sync/src/obs.rs`'s tests:

```rust
    #[test]
    fn a_registered_job_publishes_its_interval_and_no_success_yet() {
        register_job("test-interval-only", 900);
        let out = render_metrics();
        assert!(out.contains(r#"job_interval_seconds{job="test-interval-only"} 900"#));
        assert!(!out.contains(r#"job_last_success_timestamp_seconds{job="test-interval-only"}"#));
    }

    #[test]
    fn a_success_stamps_the_clock_and_a_failure_counts() {
        register_job("test-both", 60);
        job_succeeded("test-both");
        job_failed("test-both");
        let out = render_metrics();
        assert!(out.contains(r#"job_errors_total{job="test-both"} 1"#));
        let stamped = out
            .lines()
            .find(|l| l.starts_with(r#"job_last_success_timestamp_seconds{job="test-both"}"#))
            .expect("the success gauge must exist");
        let value: i64 = stamped.rsplit(' ').next().unwrap().parse().unwrap();
        assert!(value > 1_700_000_000, "expected a unix timestamp, got {value}");
    }
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test register_job
```

Expected: compile error — `cannot find function 'register_job'`.

- [ ] **Step 3: Implement the job helpers**

Append to `sync/src/obs.rs`:

```rust
pub fn register_job(job: &str, interval_secs: u64) {
    metrics()
        .job_interval
        .with_label_values(&[job])
        .set(interval_secs as i64);
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// only ever called after a run that actually succeeded — the silent-worker
/// alert reads this gauge, so stamping it on a failure would hide the failure.
pub fn job_succeeded(job: &str) {
    metrics()
        .job_last_success
        .with_label_values(&[job])
        .set(now_secs());
}

pub fn job_failed(job: &str) {
    metrics().job_errors.with_label_values(&[job]).inc();
}
```

- [ ] **Step 4: Run to verify it passes**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test obs::tests
```

Expected: all pass.

- [ ] **Step 5: Make `refresh` report whether it succeeded**

Read `sync/src/catalog.rs` around lines 405-470 first. `refresh` currently
returns `()` and logs its own failures with `println!`. Change its signature to
return `bool` — `true` only on a run that replaced or confirmed the served
payload, `false` on every early return that keeps the old payload.

Concretely, each existing `println!` that precedes an early return becomes a
`tracing::warn!` (or `error!` where it is a real failure) plus `return false`,
and the successful tail returns `true`.

Every `println!("catalog: …")` in the file becomes a `tracing` call with a
constant message and fields, for example:

```rust
// before
println!("catalog: page {page} failed ({e})");
// after
tracing::warn!(service = %crate::obs::service(), page = page, err = %e, "catalog page failed");
```

```rust
// before
println!("catalog: loaded {} stations from disk", p.stations);
// after
tracing::info!(service = %crate::obs::service(), stations = p.stations, "catalog loaded from disk");
```

Apply the same shape to all of them: constant lower-case `msg`, values as
fields, `err` for error strings.

- [ ] **Step 6: Wire the refresher to the job metrics**

Replace `spawn_refresher` in `sync/src/catalog.rs`:

```rust
/// the background refresher. the sleep is at the end so a boot fetches at once
/// rather than serving nothing for the first day — which is also what the
/// silent-worker alert needs, it tolerates only 2*interval+10m of a zero gauge.
pub fn spawn_refresher(catalogue: Arc<Catalogue>) {
    crate::obs::register_job("catalog", REFRESH_SECS);
    tokio::spawn(async move {
        catalogue.load_from_disk().await;
        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let started = std::time::Instant::now();
            match catalogue.refresh(now).await {
                true => {
                    crate::obs::job_succeeded("catalog");
                    tracing::info!(
                        service = %crate::obs::service(), job = "catalog",
                        duration_ms = started.elapsed().as_millis() as u64, "job ok"
                    );
                }
                false => {
                    crate::obs::job_failed("catalog");
                    tracing::error!(
                        service = %crate::obs::service(), job = "catalog",
                        duration_ms = started.elapsed().as_millis() as u64, "job failed"
                    );
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(REFRESH_SECS)).await;
        }
    });
}
```

- [ ] **Step 7: Fix the tests that call `refresh`**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test
```

Existing tests in `catalog.rs` call `refresh` and ignore its result; the
compiler will flag the ones that now need `let _ =` or an assertion. Where a
test already asserts the served payload changed, assert the returned `bool`
too — it is the same claim, now cheap to check.

- [ ] **Step 8: Run the full suite**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test
```

Expected: all pass.

- [ ] **Step 9: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo fmt
cargo clippy --all-targets
cargo test
git add src/obs.rs src/catalog.rs
git commit -m "report catalogue refresh health so a stalled refresh is noticed"
```

---

## Task 6: sync — `/healthz`, the private metrics server, and deleting the old one

**Files:**
- Modify: `sync/src/main.rs` (the `main` fn, the router, delete the `metrics` handler)
- Modify: `sync/src/store.rs` (add `ping`)
- Test: `sync/src/store.rs` inline

**Interfaces:**
- Consumes: `obs::track`, `obs::render_metrics`, `obs::set_build_info`, `obs::init_logging`
- Produces: `store::Store::ping(&self) -> Result<(), String>`

- [ ] **Step 1: Write the failing test for `ping`**

In `sync/src/store.rs`'s test module (read the file first to match how existing
tests open a temp database):

```rust
    #[test]
    fn ping_succeeds_on_an_open_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let s = open(path.to_str().unwrap());
        assert!(s.ping().is_ok());
    }
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test ping_succeeds
```

Expected: compile error — no method `ping`.

- [ ] **Step 3: Implement `ping`**

In `sync/src/store.rs`, in the same `impl Store` block as `stats`:

```rust
    /// the cheapest possible proof the database still answers. /healthz calls
    /// this, so it must never scan a table.
    pub fn ping(&self) -> Result<(), String> {
        let c = self.conn.lock().map_err(|e| e.to_string())?;
        c.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
```

- [ ] **Step 4: Run to verify it passes**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test ping_succeeds
```

Expected: 1 passed.

- [ ] **Step 5: Delete the old metrics handler and add the new handlers**

In `sync/src/main.rs`, delete the whole `async fn metrics(...)` function (the
one building the string with `format!`), and add:

```rust
/// the version the binary was built as; overridable at deploy time so a
/// rebuild-less redeploy can still report the right thing.
fn version() -> String {
    std::env::var("APP_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

async fn healthz(State(s): State<AppState>) -> impl IntoResponse {
    let mut checks = serde_json::Map::new();
    let db = s.store.ping();
    if let Err(e) = &db {
        tracing::error!(service = %obs::service(), check = "db", err = %e, "healthz check failed");
    }
    checks.insert(
        "db".into(),
        serde_json::Value::String(match db.is_ok() {
            true => "ok".into(),
            false => "fail".into(),
        }),
    );
    let ok = db.is_ok();
    let body = serde_json::json!({
        "status": match ok { true => "ok", false => "degraded" },
        "checks": checks,
        "version": version(),
    });
    let code = match ok {
        true => StatusCode::OK,
        false => StatusCode::SERVICE_UNAVAILABLE,
    };
    (code, Json(body))
}

/// the business gauges are read on scrape rather than maintained on every
/// write: they all come from cheap aggregates the store already computes.
async fn refresh_business_metrics(s: &AppState) {
    let (accounts, favs, last) = s.store.stats();
    let streams = s.mirror.active_streams().await;
    let plays = s.mirror.play_count();
    let (stations, bytes, refreshed_at) = match s.catalogue.get().await {
        None => (0, 0, 0),
        Some(p) => (p.stations, p.gzipped.len(), p.refreshed_at),
    };
    obs::set_business(&[
        ("sync_accounts_total", accounts as i64),
        ("sync_favs_total", favs as i64),
        ("sync_last_activity_seconds", last),
        ("sync_play_events_total", plays as i64),
        ("sync_active_streams", streams as i64),
        ("sync_catalog_stations", stations as i64),
        ("sync_catalog_bytes", bytes as i64),
        ("sync_catalog_refreshed_seconds", refreshed_at as i64),
    ]);
}

async fn metrics(State(s): State<AppState>) -> impl IntoResponse {
    refresh_business_metrics(&s).await;
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4")],
        obs::render_metrics(),
    )
}
```

- [ ] **Step 6: Add `obs::set_business` to `obs.rs`**

The business gauges are per-service and named at call time, so one registry
entry per name, created on demand:

```rust
use prometheus::IntGauge;
use std::collections::HashMap;
use std::sync::Mutex;

static BUSINESS: OnceLock<Mutex<HashMap<String, IntGauge>>> = OnceLock::new();

/// sets simple unlabelled business gauges by name, registering each the first
/// time it is seen. business metrics keep a project prefix by contract.
pub fn set_business(values: &[(&str, i64)]) {
    let map = BUSINESS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = match map.lock() {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(service = %service(), err = %e, "business gauge lock poisoned");
            return;
        }
    };
    for (name, value) in values {
        let gauge = map.entry(name.to_string()).or_insert_with(|| {
            let g = IntGauge::new(*name, *name).expect("business gauge");
            if let Err(e) = registry().register(Box::new(g.clone())) {
                tracing::error!(service = %service(), metric = %name, err = %e, "business gauge not registered");
            }
            g
        });
        gauge.set(*value);
    }
}
```

Add a test in `obs.rs`:

```rust
    #[test]
    fn a_business_gauge_is_registered_once_and_updated() {
        set_business(&[("test_widget_total", 3)]);
        set_business(&[("test_widget_total", 7)]);
        let out = render_metrics();
        assert_eq!(
            out.lines().filter(|l| l.starts_with("test_widget_total ")).count(),
            1
        );
        assert!(out.contains("test_widget_total 7"));
    }
```

- [ ] **Step 7: Rewrite `main` to run two servers**

Replace the body of `main` in `sync/src/main.rs`:

```rust
#[tokio::main]
async fn main() {
    obs::init_logging("sync");
    obs::set_build_info(&version());

    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8138".to_string());
    let metrics_bind =
        std::env::var("METRICS_ADDR").unwrap_or_else(|_| "0.0.0.0:8148".to_string());
    let db = std::env::var("SYNC_DB").unwrap_or_else(|_| "/data/sync.db".to_string());
    let catalog_file =
        std::env::var("CATALOG_FILE").unwrap_or_else(|_| "/data/catalog.json".to_string());
    let state = AppState {
        store: Arc::new(store::open(&db)),
        mirror: Arc::new(mirror::Mirror::new()),
        catalogue: Arc::new(catalog::Catalogue::new(catalog_file.into())),
    };
    catalog::spawn_refresher(state.catalogue.clone());

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/healthz", get(healthz))
        .route("/account", post(create_account).delete(delete_account))
        .route("/sync", get(get_sync).put(put_sync))
        .route("/play", post(post_play))
        .route("/now", get(get_now))
        .route("/events", get(get_events))
        .route("/catalog", get(get_catalog))
        .route("/catalog/delta", get(get_catalog_delta))
        .layer(axum::middleware::from_fn(obs::track))
        .with_state(state.clone());

    // metrics live on their own listener, published to loopback only: the
    // public mux is proxied by nginx and one missing location block would put
    // /metrics on the internet.
    let metrics_app = Router::new()
        .route("/metrics", get(metrics))
        .with_state(state);

    let metrics_listener = tokio::net::TcpListener::bind(&metrics_bind)
        .await
        .expect("bind metrics");
    tokio::spawn(async move {
        if let Err(e) = axum::serve(metrics_listener, metrics_app).await {
            tracing::error!(service = %obs::service(), err = %e, "metrics server stopped");
        }
    });

    tracing::info!(service = %obs::service(), bind = %bind, metrics_bind = %metrics_bind, "listening");
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
```

`AppState` must derive `Clone` — it already does.

Note the middleware is `.layer(...)` **after** the routes so `MatchedPath` is
populated by the time `track` reads it, and `/metrics` deliberately has no
`track` layer — scrapes are not traffic worth counting as traffic.

- [ ] **Step 8: Convert the remaining println! in sync**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
grep -rn "println!\|eprintln!" src/
```

Every hit becomes a `tracing` call with a constant message and fields, in the
shape shown in Task 5 Step 5. Expected remaining hits after Task 5: the
`listening` line in `main` (already replaced above) and any in `mirror.rs` /
`merge.rs` / `store.rs`.

- [ ] **Step 9: Run the full suite**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo test
```

Expected: all pass.

- [ ] **Step 10: Verify the two servers by hand**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
SYNC_DB=/tmp/t.db CATALOG_FILE=/tmp/t.json BIND_ADDR=127.0.0.1:18138 \
  METRICS_ADDR=127.0.0.1:18148 LOG_LEVEL=INFO cargo run --release &
sleep 3
curl -si 127.0.0.1:18138/healthz
curl -s 127.0.0.1:18138/health
curl -s 127.0.0.1:18148/metrics | grep -E "^(http_requests_total|build_info|job_interval_seconds|sync_accounts_total)" 
curl -si 127.0.0.1:18138/metrics | head -1
kill %1
```

Expected:
- `/healthz` → `200` with `{"status":"ok","checks":{"db":"ok"},"version":"…"}` and an `x-request-id` header
- `/metrics` on **18148** → contract metric lines present
- `/metrics` on **18138** → `404` (it moved off the public mux)
- the log lines on stdout are single-line JSON with `level` and `message`

- [ ] **Step 11: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo fmt
cargo clippy --all-targets
cargo test
git add -A src/
git commit -m "serve health and metrics the way the monitoring stack expects"
```

---

## Task 7: stat — the same obs module

**Files:**
- Create: `stat/src/obs.rs`
- Modify: `stat/Cargo.toml`
- Modify: `stat/src/main.rs:1-10` (add `mod obs;`)

**Interfaces:**
- Produces: exactly the same public surface as sync's `obs.rs` — `init_logging`, `service`, `registry`, `render_metrics`, `set_build_info`, `observe_request`, `set_business`, `safe_request_id`, `mint_request_id`, `route_of`, `track`, `register_job`, `job_succeeded`, `job_failed`

The two copies are deliberate: `sync` and `stat` are separate repos with no
shared crate, and inventing one is out of scope. Copying ~250 lines is the
cheaper honest option.

- [ ] **Step 1: Add the dependencies**

In `stat/Cargo.toml`, under `[dependencies]`:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
prometheus = { version = "0.14", default-features = false }
rand = "0.8"
hex = "0.4"
```

`rand` and `hex` are new here — sync already had them; `mint_request_id` needs
both.

- [ ] **Step 2: Copy the module**

```bash
cd /Users/vchub/dev/projects/world-radio
cp sync/src/obs.rs stat/src/obs.rs
```

Add `mod obs;` to the top of `stat/src/main.rs` alongside the other `mod` lines.

- [ ] **Step 3: Run the tests**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo test obs::tests
```

Expected: all the obs tests pass unchanged. If anything fails to compile, it is
a missing dependency from Step 1 — fix that, do not edit the module.

- [ ] **Step 4: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo fmt
cargo clippy --all-targets
cargo test
git add Cargo.toml Cargo.lock src/obs.rs src/main.rs
git commit -m "log as json and expose the standard metrics"
```

---

## Task 8: stat — delete the admin page and the sync scrape

**Files:**
- Delete: `stat/src/render.rs`
- Delete: `stat/src/sync_stats.rs`
- Modify: `stat/src/main.rs` (drop `mod render`, `mod sync_stats`, the `/stat` route, the `stat` handler, `AppState.sync`, the `sync_url` env and its poll block)
- Modify: `stat/src/aggregate.rs` (string builders → gauges)
- Modify: `ops/site/**` if it links `/stat`

**Interfaces:**
- Consumes: `obs::set_business`
- Produces:
  - `aggregate::download_counts(events: &[DownloadEvent]) -> Vec<((String, String), u64)>` — `((country, version), count)`, sorted
  - `aggregate::publish(events, apk, cli, app)` — sets every stat business gauge

- [ ] **Step 1: Write the failing test**

Replace the tests in `stat/src/aggregate.rs` (the two `render_*` tests go with
the functions they test) with:

```rust
    #[test]
    fn counts_by_country_and_version() {
        let events = vec![ev("PL", "1.1.0"), ev("PL", "1.1.0"), ev("DE", "1.1.0")];
        let counts = download_counts(&events);
        assert_eq!(
            counts,
            vec![
                (("DE".to_string(), "1.1.0".to_string()), 1),
                (("PL".to_string(), "1.1.0".to_string()), 2),
            ]
        );
    }

    #[test]
    fn no_events_means_no_counts() {
        assert!(download_counts(&[]).is_empty());
    }
```

Keep the existing `ev` helper.

- [ ] **Step 2: Run to verify it fails**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo test download_counts
```

Expected: compile error — `cannot find function 'download_counts'`.

- [ ] **Step 3: Rewrite `aggregate.rs`**

Replace `render_metrics` and `render_apk_metrics` with:

```rust
use crate::log_parser::DownloadEvent;
use crate::obs;
use prometheus::{IntCounterVec, Opts};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// download totals are cumulative counts read from a log and from github, so
/// they are published as counters set to an absolute value — a labelled gauge
/// would be wrong for a rate() query and a real counter cannot be set.
/// prometheus has no "set" on a counter, so these are IntGaugeVec carrying
/// _total names, which the text format accepts and rate() handles because the
/// series only ever grows.
fn downloads() -> &'static prometheus::IntGaugeVec {
    static M: OnceLock<prometheus::IntGaugeVec> = OnceLock::new();
    M.get_or_init(|| {
        let m = prometheus::IntGaugeVec::new(
            Opts::new("downloads_total", "cli binary downloads seen in the release log"),
            &["country", "version"],
        )
        .expect("downloads_total");
        obs::registry()
            .register(Box::new(m.clone()))
            .expect("register downloads_total");
        m
    })
}

fn github_downloads() -> &'static prometheus::IntGaugeVec {
    static M: OnceLock<prometheus::IntGaugeVec> = OnceLock::new();
    M.get_or_init(|| {
        let m = prometheus::IntGaugeVec::new(
            Opts::new(
                "github_downloads_total",
                "release asset downloads reported by github",
            ),
            &["kind", "version"],
        )
        .expect("github_downloads_total");
        obs::registry()
            .register(Box::new(m.clone()))
            .expect("register github_downloads_total");
        m
    })
}

pub fn download_counts(events: &[DownloadEvent]) -> Vec<((String, String), u64)> {
    let mut counts: BTreeMap<(String, String), u64> = BTreeMap::new();
    for e in events {
        *counts
            .entry((e.country.clone(), e.version.clone()))
            .or_insert(0) += 1;
    }
    counts.into_iter().collect()
}

/// publishes every download figure onto the registry. kind separates the three
/// github asset families so one metric name serves all of them.
pub fn publish(
    events: &[DownloadEvent],
    apk: &[(String, u64)],
    cli: &[(String, u64)],
    app: &[(String, u64)],
) {
    for ((country, version), n) in download_counts(events) {
        downloads()
            .with_label_values(&[&country, &version])
            .set(n as i64);
    }
    for (kind, list) in [("apk", apk), ("cli", cli), ("app", app)] {
        for (version, n) in list {
            github_downloads()
                .with_label_values(&[kind, version])
                .set(*n as i64);
        }
    }
}
```

Note this replaces the spec's three separate `github_*_downloads_total` names
with one `github_downloads_total{kind,version}` — three enumerable kinds, one
metric, same information, fewer names to maintain. Record that change in the
spec file when committing.

- [ ] **Step 4: Add a test for `publish`**

```rust
    #[test]
    fn publish_exposes_every_download_family() {
        publish(
            &[ev("PL", "1.1.0")],
            &[("1.4.20".into(), 5)],
            &[("1.4.20".into(), 9)],
            &[("1.4.20".into(), 2)],
        );
        let out = obs::render_metrics();
        assert!(out.contains(r#"downloads_total{country="PL",version="1.1.0"} 1"#));
        assert!(out.contains(r#"github_downloads_total{kind="apk",version="1.4.20"} 5"#));
        assert!(out.contains(r#"github_downloads_total{kind="cli",version="1.4.20"} 9"#));
        assert!(out.contains(r#"github_downloads_total{kind="app",version="1.4.20"} 2"#));
    }
```

- [ ] **Step 5: Run to verify it passes**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo test aggregate
```

Expected: all pass.

- [ ] **Step 6: Delete the admin page and the sync scrape**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
git rm src/render.rs src/sync_stats.rs
```

In `stat/src/main.rs` remove:
- `mod render;` and `mod sync_stats;`
- the `sync: Arc<RwLock<Option<...>>>` field from `AppState` and its initialiser
- `let poll_sync = state.sync.clone();`
- the `sync_url` env read and the `if let Some(stats) = sync_stats::fetch(...)` block
- the `.route("/stat", get(stat))` line and the whole `async fn stat(...)` handler
- the `StatQuery` type if nothing else uses it — check with `grep -n StatQuery src/`

- [ ] **Step 7: Check whether the site links the removed page**

```bash
cd /Users/vchub/dev/projects/world-radio
grep -rn "/stat" ops/site ops/deploy 2>/dev/null | grep -v node_modules
```

If the site or an nginx config links or proxies `/stat`, remove that link in the
`ops` repo and commit there separately — `ops` is its own repo. If the nginx
route lives only in `ops/deploy/nginx/*.conf`, note that those files are
documentation and are **edited by hand on the host**; removing the route
requires a host edit, not just a commit.

- [ ] **Step 8: Run the full suite**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo test
```

Expected: all pass. The `render.rs` tests are gone with the file.

- [ ] **Step 9: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo fmt
cargo clippy --all-targets
cargo test
git add -A
git commit -m "retire the stat page in favour of the grafana dashboard"
```

---

## Task 9: stat — health, metrics server, job metrics, download log lines

**Files:**
- Modify: `stat/src/main.rs`
- Modify: `stat/src/notify.rs`, `stat/src/github.rs`, `stat/src/store.rs` (println → tracing)

**Interfaces:**
- Consumes: everything from `obs`, `aggregate::publish`
- Produces: nothing new for later tasks

- [ ] **Step 1: Add `version`, `healthz` and the metrics handler**

In `stat/src/main.rs`:

```rust
fn version() -> String {
    std::env::var("APP_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

async fn healthz(State(s): State<AppState>) -> impl IntoResponse {
    // the release log is the only hard dependency: without it this service has
    // nothing to report. github is an upstream, not a dependency — it has its
    // own job metrics.
    let readable = std::fs::metadata(s.log_path.as_path()).is_ok();
    if !readable {
        tracing::error!(
            service = %obs::service(), check = "release_log",
            path = %s.log_path.display(), "healthz check failed"
        );
    }
    let body = serde_json::json!({
        "status": match readable { true => "ok", false => "degraded" },
        "checks": { "release_log": match readable { true => "ok", false => "fail" } },
        "version": version(),
    });
    let code = match readable {
        true => StatusCode::OK,
        false => StatusCode::SERVICE_UNAVAILABLE,
    };
    (code, Json(body))
}

async fn metrics(State(s): State<AppState>) -> impl IntoResponse {
    let res = store::read_events(
        &s.log_path,
        &store::Filter {
            country: None,
            version: None,
            limit: usize::MAX,
        },
    );
    let apk = s.apk.read().await;
    let cli = s.cli_downloads.read().await;
    let app = s.app_downloads.read().await;
    aggregate::publish(&res.events, &apk, &cli, &app);
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        obs::render_metrics(),
    )
}
```

`StatusCode` and `Json` need importing from `axum::http` / `axum` — check the
existing imports at the top of the file and add what is missing.

- [ ] **Step 2: Instrument the github poll as a job**

The poll loop in `main` becomes a job with the contract's metrics. Wrap its body
so a run counts as failed when github gave nothing at all:

```rust
    obs::register_job("github-poll", 900);
    tokio::spawn(async move {
        loop {
            let started = std::time::Instant::now();
            let (apk_counts, cli_counts, app_counts) =
                github::fetch_download_counts(token.as_deref()).await;
            // a run that got nothing from any of the three is a failed run —
            // github erroring or rate-limiting us looks exactly like this.
            let got_anything =
                !apk_counts.is_empty() || !cli_counts.is_empty() || !app_counts.is_empty();

            if !apk_counts.is_empty() {
                *poll_apk.write().await = apk_counts;
            }
            if !cli_counts.is_empty() {
                *poll_cli_downloads.write().await = cli_counts;
            }
            if !app_counts.is_empty() {
                *poll_app_downloads.write().await = app_counts;
            }
            if let Some(info) = github::fetch_android_info(token.as_deref()).await {
                *poll_android.write().await = Some(info);
            }
            if let Some(info) = github::fetch_cli_info(token.as_deref()).await {
                *poll_cli.write().await = Some(info);
            }
            if let Some(cfg) = &notify_cfg {
                announce_new_downloads(cfg, &notify_log, &notify_dir).await;
            }

            let duration_ms = started.elapsed().as_millis() as u64;
            match got_anything {
                true => {
                    obs::job_succeeded("github-poll");
                    tracing::info!(service = %obs::service(), job = "github-poll", duration_ms, "job ok");
                }
                false => {
                    obs::job_failed("github-poll");
                    tracing::error!(service = %obs::service(), job = "github-poll", duration_ms, "job failed");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(900)).await;
        }
    });
```

Keep the existing `poll_sync` removal from Task 8 — it is already gone.

- [ ] **Step 3: Emit the download events to stdout**

**Read `stat/src/main.rs:39-59` and `stat/src/notify.rs:46-72` before editing.**
The real code differs from what an earlier draft of this plan assumed: the
helper is `notify::unreported(&events, &mark) -> Vec<&DownloadEvent>` (not
`fresh_events`), the mark path comes from `notify::mark_path(notify_dir)` (there
is no `MARK_FILE` constant), and `notify::messages` takes `&[&DownloadEvent]`.

The current function has two early returns that must not swallow the log lines:
a first run with no mark returns before announcing anything, and an empty
`fresh` returns too. Download lines must reach Loki on every path where fresh
events exist, and telegram must stay optional.

Replace the whole function with:

```rust
/// one poll's worth of announcing. the whole log is re-read every time, so the
/// mark is what stops it re-announcing everything; a first run with no mark
/// sends nothing at all, because a fresh deployment must not open with a burst
/// of old news.
///
/// cfg is optional: the download lines below are what replaces the old stat
/// page's "recent downloads" table, so they must be written whether or not
/// telegram is configured.
async fn announce_new_downloads(
    cfg: Option<&notify::Config>,
    log_path: &Path,
    notify_dir: &Path,
) {
    let events = store::read_events(log_path, &store::Filter::default()).events;
    let mark_file = notify::mark_path(notify_dir);
    let Some(mark) = notify::load_mark(&mark_file) else {
        notify::save_mark(&mark_file, &notify::advance_mark(&events, ""));
        tracing::info!(service = %obs::service(), "notify first run, starting from the newest entry");
        return;
    };
    let fresh = notify::unreported(&events, &mark);
    if fresh.is_empty() {
        return;
    }
    // one line per new download: this is what replaces the "recent downloads"
    // table. loki keeps them for 14 days and grafana can filter them. never
    // the ip or the user agent.
    for e in &fresh {
        tracing::info!(
            service = %obs::service(), file = %e.file, version = %e.version,
            country = %e.country, "download"
        );
    }
    if let Some(cfg) = cfg {
        for text in notify::messages(&fresh) {
            notify::send(cfg, &text).await;
        }
    }
    notify::save_mark(&mark_file, &notify::advance_mark(&events, &mark));
}
```

Note the first-run branch still returns early and logs nothing per-event. That
is correct and deliberate: on a fresh deployment the whole historical log would
otherwise be replayed into Loki as if it had just happened.

- [ ] **Step 3b: Call it on every poll, not only when telegram is on**

In the poll loop, the call is currently inside `if let Some(cfg) = &notify_cfg`.
Move it out so it runs unconditionally:

```rust
            announce_new_downloads(notify_cfg.as_ref(), &notify_log, &notify_dir).await;
```

- [ ] **Step 4: Rewrite the router and `main`'s tail**

```rust
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/healthz", get(healthz))
        .route("/android-info", get(android_info))
        .route("/cli-info", get(cli_info))
        .layer(axum::middleware::from_fn(obs::track))
        .with_state(state.clone());

    let metrics_app = Router::new()
        .route("/metrics", get(metrics))
        .with_state(state);

    let metrics_bind =
        std::env::var("METRICS_ADDR").unwrap_or_else(|_| "0.0.0.0:8147".to_string());
    let metrics_listener = tokio::net::TcpListener::bind(&metrics_bind)
        .await
        .expect("bind metrics");
    tokio::spawn(async move {
        if let Err(e) = axum::serve(metrics_listener, metrics_app).await {
            tracing::error!(service = %obs::service(), err = %e, "metrics server stopped");
        }
    });

    tracing::info!(service = %obs::service(), bind = %bind, metrics_bind = %metrics_bind, "listening");
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
```

and at the very top of `main`:

```rust
    obs::init_logging("stat");
    obs::set_build_info(&version());
```

`AppState` must derive `Clone` — add it if it does not.

- [ ] **Step 5: Convert the remaining println!/eprintln! in stat**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
grep -rn "println!\|eprintln!" src/
```

Convert each, constant message plus fields:

```rust
// before
eprintln!("notify: telegram refused the message: {}", r.status());
// after
tracing::warn!(service = %crate::obs::service(), status = r.status().as_u16(), "telegram refused the message");
```

```rust
// before
println!("notify: off (no telegram token or chat id)");
// after
tracing::info!(service = %obs::service(), "telegram notifications off");
```

- [ ] **Step 6: Run the full suite**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo test
```

Expected: all pass.

- [ ] **Step 7: Verify by hand**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
printf '{"time":"2026-08-22T10:00:00+00:00","file":"/releases/r4dio-1.22.4-x86_64-unknown-linux-gnu.tar.gz","status":200,"ip":"203.0.113.7","country":"PL","ua":"curl/8.0"}\n' > /tmp/rel.log
RELEASE_LOG=/tmp/rel.log NOTIFY_DIR=/tmp BIND_ADDR=127.0.0.1:18137 \
  METRICS_ADDR=127.0.0.1:18147 LOG_LEVEL=INFO cargo run --release &
sleep 5
curl -si 127.0.0.1:18137/healthz
curl -s 127.0.0.1:18147/metrics | grep -E "^(downloads_total|build_info|job_interval_seconds|http_requests_total)"
curl -si 127.0.0.1:18137/stat | head -1
kill %1
```

Expected:
- `/healthz` → 200, `checks.release_log = "ok"`
- `/metrics` on 18147 → `downloads_total{country="PL",version="1.22.4"} 1` plus the contract metrics
- `/stat` → **404**, the page is gone
- one stdout JSON line with `"message":"download"` and no `ip` or `ua` key

Confirm no ip/ua leaked:
```bash
# in the captured output above
```
The download line must contain `country`, `version`, `file` and nothing else
from the event.

- [ ] **Step 8: Format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/stat
cargo fmt
cargo clippy --all-targets
cargo test
git add -A src/
git commit -m "report health, downloads and poll freshness as metrics"
```

---

## Task 10: compose — one project name, private metrics ports, health

**Files:**
- Modify: `sync/docker-compose.yml`
- Modify: `stat/docker-compose.yml`
- Verify: `ops/deploy/docker-compose.prod.yml`

**Interfaces:**
- Consumes: the volume names recorded in Task 1

**This task can strand production data if Task 1's findings are not used.**
Adding `name: world-radio` renames unpinned volumes. Do not proceed without the
recorded names.

- [ ] **Step 1: Rewrite `sync/docker-compose.yml`**

The volume name below is the real one, read off the running container on 2026-08-22. Do not change it.

```yaml
# one project name across every compose file in world-radio: loki takes the
# project label from com.docker.compose.project, so this is what makes sync,
# stat and web one project in grafana rather than three.
name: world-radio

services:
  sync:
    build: .
    container_name: world-radio-sync
    restart: unless-stopped
    ports:
      - "127.0.0.1:8138:8138"
      # metrics are loopback-only and never proxied by nginx
      - "127.0.0.1:8148:8148"
    environment:
      - SYNC_DB=/data/sync.db
      - BIND_ADDR=0.0.0.0:8138
      - METRICS_ADDR=0.0.0.0:8148
      - LOG_LEVEL=${LOG_LEVEL:-INFO}
      - APP_VERSION=${APP_VERSION:-}
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:8138/healthz"]
      interval: 30s
      timeout: 3s
      retries: 3
    volumes:
      - sync-data:/data

volumes:
  sync-data:
    # pinned: adding the project name above would otherwise rename this volume
    # and strand the accounts database.
    name: world-radio-sync_sync-data
```

The `172.17.0.1:8138` publish is gone — it existed only so stat could scrape
sync, which Task 8 deleted.

**The healthcheck binary must exist in the runtime image, and it probably does
not.** Both Dockerfiles use `debian:bookworm-slim`, which ships neither `wget`
nor `curl` — the `apt-get install` line in each installs only `ca-certificates`.
A healthcheck calling a missing binary makes the container permanently
`unhealthy`, which is worse than having no healthcheck.

Verify before trusting either:

```bash
docker run --rm debian:bookworm-slim sh -c 'command -v wget || command -v curl || echo NEITHER'
```

If it prints `NEITHER` (expected), add `curl` to the runtime stage of **both**
`sync/Dockerfile` and `stat/Dockerfile`:

```dockerfile
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
```

and use `curl` in both healthchecks instead of `wget`:

```yaml
    healthcheck:
      test: ["CMD", "curl", "-fsS", "http://localhost:8138/healthz"]
```

(port 8137 for stat). Commit the Dockerfile change together with the compose
change in Step 5.

- [ ] **Step 2: Rewrite `stat/docker-compose.yml`**

The volume name below is the real one, read off the running container on 2026-08-22. Do not change it.

```yaml
name: world-radio

services:
  stat:
    build: .
    container_name: world-radio-stat
    restart: unless-stopped
    ports:
      - "127.0.0.1:8137:8137"
      - "127.0.0.1:8147:8147"
    environment:
      - RELEASE_LOG=/logs/releases.log
      - NOTIFY_DIR=/data
      - TELEGRAM_BOT_TOKEN=${TELEGRAM_BOT_TOKEN:-}
      - TELEGRAM_CHAT_ID=${TELEGRAM_CHAT_ID:-}
      - BIND_ADDR=0.0.0.0:8137
      - METRICS_ADDR=0.0.0.0:8147
      - GITHUB_TOKEN=${GITHUB_TOKEN:-}
      - LOG_LEVEL=${LOG_LEVEL:-INFO}
      - APP_VERSION=${APP_VERSION:-}
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:8137/healthz"]
      interval: 30s
      timeout: 3s
      retries: 3
    volumes:
      - /var/log/nginx/world-radio-releases.log:/logs/releases.log:ro
      # the notify mark must outlive a redeploy, or every deploy replays the log
      - stat-data:/data

volumes:
  stat-data:
    name: world-radio-stat_stat-data
```

`SYNC_METRICS_URL` is gone with the scrape.

- [ ] **Step 3: Validate both files parse and resolve to the pinned volumes**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
docker compose config | grep -A3 "^volumes:"
cd ../stat
docker compose config | grep -A3 "^volumes:"
```

Expected: each prints the **existing** volume name from Task 1, not a
`world-radio_`-prefixed new one.

- [ ] **Step 4: Confirm the ops compose already carries the name**

```bash
cd /Users/vchub/dev/projects/world-radio
head -3 ops/deploy/docker-compose.prod.yml
```

Expected: `name: world-radio` on line 1. If it is there, `ops` needs no compose
change.

- [ ] **Step 5: Commit each repo separately**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
git add docker-compose.yml
git commit -m "run under the shared project name with a private metrics port"

cd ../stat
git add docker-compose.yml
git commit -m "run under the shared project name with a private metrics port"
```

---

## Task 11: deploy, then repoint mon

**Files:**
- Modify: `mon/prometheus/prometheus.yml`

This task is last on purpose: repointing Prometheus before the new ports are
live turns the world-radio targets red.

- [ ] **Step 1: Push both services**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
git push origin dev
cd ../stat
git push origin dev
```

Each repo's own Deploy workflow builds and deploys it. Do not deploy sync from
ops — `ops/.github/workflows/deploy.yml` records why.

- [ ] **Step 2: Wait for both deploys and verify on the host**

```bash
docker compose ls
docker ps --filter name=world-radio
curl -s 127.0.0.1:8147/metrics | grep -c http_requests_total
curl -s 127.0.0.1:8148/metrics | grep -c http_requests_total
curl -s 127.0.0.1:8137/healthz
curl -s 127.0.0.1:8138/healthz
```

Expected: `docker compose ls` shows **one** `world-radio` project; both greps
return non-zero; both `/healthz` return `{"status":"ok",...}`.

If `docker compose ls` still shows separate `sync`/`stat` projects, the old
containers were not recreated — `docker compose up -d --remove-orphans` from
each directory on the host.

- [ ] **Step 3: Confirm the old surfaces are gone**

```bash
curl -si 127.0.0.1:8137/stat | head -1
curl -si 127.0.0.1:8137/metrics | head -1
curl -si 127.0.0.1:8138/metrics | head -1
```

Expected: all three `404`.

Also confirm the public site does not expose the metrics ports:
```bash
curl -si https://r4dio.net/metrics | head -1
```
Expected: not a metrics body.

- [ ] **Step 4: Confirm logs reached Loki**

In Grafana (`mon.vchub.net`), Explore → Loki:

```logql
{project="world-radio"}
{project="world-radio", service="stat", level="INFO"}
{project="world-radio", service="stat"} | json | msg="download"
```

Expected: lines from both `stat` and `sync`, with a working `level` label. If
`level` is absent, the json is not being parsed — check that lines are single
JSON objects with a `level` key.

The `msg="download"` query may return nothing until a real download happens;
that is expected, not a failure. Confirm the field exists by checking the
service started and the mark file logic ran at least once.

- [ ] **Step 5: Open the PR against mon**

```bash
cd /Users/vchub/dev/projects/mon
git checkout -b repoint-world-radio-metrics
```

In `prometheus/prometheus.yml`, change only the two target ports:

```yaml
  - job_name: world-radio-stat
    metrics_path: /metrics
    static_configs:
      - targets:
          - host.docker.internal:8147
        labels:
          project: world-radio
          service: stat

  - job_name: world-radio-sync
    metrics_path: /metrics
    static_configs:
      - targets:
          - host.docker.internal:8148
        labels:
          project: world-radio
          service: sync
```

The blackbox target for `https://r4dio.net` already exists and is already
labelled `project: world-radio`. Point it at `/healthz` only if r4dio.net
serves one — the web container is nginx serving a static site, so leave it at
`https://r4dio.net`.

```bash
git add prometheus/prometheus.yml
git commit -m "scrape world-radio on its private metrics ports"
git push -u origin repoint-world-radio-metrics
gh pr create --base main --title "scrape world-radio on its private metrics ports" \
  --body "world-radio moved /metrics off the public mux onto loopback-only listeners: stat 8137 → 8147, sync 8138 → 8148. Both are live and serving the contract metric names.

## What's New
- world-radio now emits the standard http, job and build metrics, so its dashboard populates."
```

- [ ] **Step 6: After the mon PR merges, verify the dashboard**

In Grafana, open the project dashboard and select `world-radio`.

Expected populated:
- health row: `up` = 1 for both services, `probe_success` for r4dio.net, `build_info` showing the version
- http row: request rate, 5xx ratio, p95 by route
- logs row: lines per level, split by service
- workers row: `catalog` and `github-poll` with a last-success age below their intervals

Also check Prometheus targets directly:
```
mon.vchub.net → Prometheus → Status → Targets
```
Expected: `world-radio-stat` and `world-radio-sync` both `UP`.

- [ ] **Step 7: Record the outcome**

Update `docs/superpowers/plans/2026-08-22-mon-onboarding-spec.md` with a short
"shipped" section: what is live, the verified evidence, and anything left open.

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add docs/superpowers/plans/2026-08-22-mon-onboarding-spec.md
git commit -m "record what the monitoring rollout verified"
git push origin dev
```

---

## Self-review notes

**Spec coverage.** Every spec section maps to a task: old surface removal →
Tasks 6, 8; logs → 2, 4, 5, 6, 9; healthz → 6, 9; metrics contract → 3, 5, 9;
download events to Loki → 9; compose/project name → 10; ports → 6, 9, 10, 11;
mon PR → 11. The spec's "not now" items (mon telegram webhook, alert authoring,
clients) have no tasks, as intended.

**One deviation from the spec, deliberate.** Task 8 collapses the spec's
`github_apk_downloads_total` / `github_cli_downloads_total` /
`github_app_downloads_total` into `github_downloads_total{kind,version}`. Three
enumerable kinds, one name. Task 8 Step 3 says to record this in the spec when
committing.

**Two risks worth naming.**

1. **Volume renaming (Task 10).** Adding `name:` renames unpinned volumes and
   would strand the sync database and the notify mark. Task 1 exists solely to
   read the real names off the host first, and Task 10 refuses to proceed
   without them.
2. **`docker compose ls` may not collapse cleanly.** Three compose files sharing
   one project name is legal and gives one Loki project, but the host currently
   has containers under the old project names. They must be recreated, which
   Task 11 Step 2 checks for explicitly.
