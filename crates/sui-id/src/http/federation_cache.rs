//! RFC 096-A stage 4d: the request/cache state table -- single flight, the
//! 30-second failure cooldown, the 60-second forced-refresh window for an
//! unknown `kid`, and publish-only-if-still-current rotation. RFC 096
//! `:897-924`. The last of 096-A's cache stages, and the only one with
//! concurrency.
//!
//! ## Map key, and why it cannot grow with attacker input
//!
//! The one map here is `HashMap<FederationProviderId, Slot<T>>`, inside
//! [`FederationDocumentCache`]. A `FederationProviderId` only ever reaches
//! this cache after resolving to a configured row in `federation_provider`
//! -- never straight from an attacker's raw input -- so this map is bounded
//! by the number of configured providers, a value an administrator
//! controls, not request traffic. No `kid` value is ever used as a map key,
//! or stored anywhere past the one `&str` comparison [`lookup_for_kid`]'s
//! caller-supplied `has_kid` closure performs; an attacker presenting a
//! thousand distinct `kid`s cannot grow anything this module owns.
//!
//! ## Clock
//!
//! A new abstraction, unlike stages 4b/4c: [`MonotonicClock`]. Those stages
//! had no stored timers ticking forward in real elapsed time; this one
//! does (the cooldown deadline, the forced-dispatch timestamp, and the
//! resident time fed to `RetainedEntry::is_fresh_after`), and
//! `std::time::Instant` cannot be mocked directly, so a trait is needed to
//! drive it in tests. `sui_id_core::time::Clock` (the wall clock 4b/4c use)
//! is not reused here and is not needed here either: every timer in this
//! module is monotonic, and the wall clock stays entirely inside
//! `store_fresh`/`apply_304`, called by whichever fetch closure a caller
//! supplies -- never by this module directly.
//!
//! ## The scheduling seam for deterministic concurrency tests
//!
//! None was added beyond what the design already has. The fetch closure
//! [`lookup`]/[`lookup_for_kid`] call is invoked strictly *after* the
//! leader state transition is committed (the lock taken, the slot set to
//! `Phase::InFlight`, the lock released) and strictly *before* that
//! transition is visible to a joiner as anything but "in flight". A test's
//! own fetch closure can therefore signal its own start as its first line
//! and block on a second signal before returning -- enough to sequence
//! "caller A is now the leader and is fetching" before spawning caller B,
//! without any cache-internal hook written for tests alone. See the test
//! module for the pattern.
//!
//! ## What is not here
//!
//! No network call is made by this module -- the fetch closure a caller
//! supplies makes it, and decides 200-vs-304 and calls `store_fresh`/
//! `apply_304` itself (both reused from stage 4c, unmodified). This module
//! owns only *when* to call that closure, and what to do with what it
//! returns.
//!
//! ## Authority on the read paths (stage 4d fix, RFC 096 `:890-891`/`:833`)
//!
//! The first version of this module consulted `current_key()` only at
//! publish time, deciding whether to *retain* a fetched result. Neither
//! read path consulted it before *serving* one: a fresh hit returned
//! whatever was retained regardless of whether it still matched the live
//! `(provider, version, activation generation)`, and a joiner joined
//! whatever flight was active regardless of what key that flight was
//! fetching for. Both are now gated:
//!
//! - **Fresh hit:** a retained [`RetainedEntry`] whose `.key()` no longer
//!   equals `current_key()` is dropped as soon as it is noticed (not left
//!   to be re-compared on every subsequent lookup) and treated as a miss.
//! - **Join:** a caller only joins the active flight in [`Phase::InFlight`]
//!   when its recorded key equals `current_key()`. Otherwise it leads a new
//!   flight for its own key, which overwrites `phase` -- the superseded
//!   flight keeps running independently and is not cancelled.
//!
//! The map is still **not** keyed on `CacheKey`: version and activation
//! generation advance over time, so that key would grow with activation
//! count rather than with configured providers, re-opening exactly the
//! unbounded-growth property this module was written to avoid. Comparing
//! before serving, rather than keying by the thing being compared, is also
//! what RFC 096 `:833` literally asks for ("non-authoritative by durable
//! version/state comparison").
//!
//! **The interaction this creates:** a flight that gets superseded (a
//! second caller's key differs from the active one and overwrites `phase`)
//! is still running in the background with its own `tx`/`flight_generation`
//! captured from before it was superseded. When it finishes, `run_flight`
//! must not blindly reset `phase` to `Idle` or touch `cooldown_until`/
//! `entry` -- doing so would clobber whichever newer flight has since taken
//! the slot over. [`Slot::flight_generation`] is a counter incremented
//! every time a flight is led, carried alongside the key on
//! [`Phase::InFlight`]; `run_flight` compares its own captured generation
//! against the slot's *current* one before touching anything. A plain key
//! comparison was considered and rejected: a key can recur (a config change
//! reverted, or toggled twice) after being superseded, so two different
//! flights can share the same `CacheKey` without being the same flight --
//! only a counter that never repeats identifies "this exact flight"
//! reliably across time. A superseded flight's result, success or failure,
//! then affects nothing about the slot; the caller who led it still
//! receives the fetched data for its own one operation, same as any other
//! discarded-but-usable result in this module.
//!
//! A related correction fell out of restructuring the fresh-hit path: the
//! table's own row 3 precondition is "stale/miss, active flight", so a
//! fresh, correctly-keyed entry is now checked *before* `phase` is examined
//! at all, rather than being bypassed whenever any flight (such as a
//! concurrent forced one from `lookup_for_kid`) happened to be active. The
//! original code joined an active flight unconditionally even when the
//! existing entry was already fresh, which the table's own wording did not
//! call for. `lookup_for_kid`'s rows 5-8 deliberately keep bypassing
//! freshness -- that bypass is row 8's whole point, not an oversight.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sui_id_shared::ids::FederationProviderId;
use tokio::sync::watch;

use crate::cache_freshness::{CacheKey, CacheOutcome, RetainedEntry};

/// RFC 096 `:897-924`: "a failed flight starts the 30-second cooldown."
const FAILURE_COOLDOWN: Duration = Duration::from_secs(30);

/// RFC 096 `:897-924`: the forced-refresh budget for an unknown `kid`,
/// measured from dispatch, not completion (see the module's rule-1 test).
const FORCED_REFRESH_WINDOW: Duration = Duration::from_secs(60);

/// A monotonic time source, injected so tests can advance it exactly rather
/// than sleeping. See the module doc comment for why this is a new
/// abstraction rather than a reuse of `sui_id_core::time::Clock`.
pub trait MonotonicClock: Send + Sync {
    fn now(&self) -> Instant;
}

pub type SharedMonotonicClock = Arc<dyn MonotonicClock>;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMonotonicClock;

impl MonotonicClock for SystemMonotonicClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub fn system_monotonic_clock() -> SharedMonotonicClock {
    Arc::new(SystemMonotonicClock)
}

/// A [`MonotonicClock`] tests advance explicitly. `Instant` has no public
/// constructor for an arbitrary value, so this anchors to one real
/// `Instant::now()` taken at construction and reports `anchor + offset`,
/// where `offset` only ever grows -- enough for every rule in this module,
/// none of which ever need anything but the *difference* between two
/// readings.
#[derive(Debug, Clone)]
pub struct MockMonotonicClock {
    anchor: Instant,
    offset: Arc<Mutex<Duration>>,
}

impl MockMonotonicClock {
    pub fn new() -> Self {
        Self {
            anchor: Instant::now(),
            offset: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut offset = lock(&self.offset);
        *offset += by;
    }
}

impl Default for MockMonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock for MockMonotonicClock {
    fn now(&self) -> Instant {
        self.anchor + *lock(&self.offset)
    }
}

/// A panic inside one caller's critical section must not permanently poison
/// this cache for every other caller -- recovering the inner value rather
/// than panicking matches `runtime::ratelimit`'s own convention for the
/// same reason.
fn lock<G>(mutex: &Mutex<G>) -> std::sync::MutexGuard<'_, G> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// What a joiner reads off the watch channel once the flight it joined
/// resolves: `None` while pending, then exactly one `Some`. The error case
/// carries no detail -- a joiner never ran the fetch closure, so it has
/// nothing more specific to report than "the flight I joined failed"
/// ([`LookupError::FlightFailed`]); the leader's own caller gets the real
/// `E` directly, never routed through this channel at all.
type FlightSignal<T> = Option<Result<(RetainedEntry<T>, CacheOutcome), ()>>;

enum Phase<T> {
    Idle,
    InFlight {
        rx: watch::Receiver<FlightSignal<T>>,
        key: CacheKey,
        generation: u64,
    },
}

struct Slot<T> {
    entry: Option<RetainedEntry<T>>,
    /// When `entry` was last published, by [`MonotonicClock`] -- never the
    /// wall clock. `None` until the first successful flight.
    stored_at: Option<Instant>,
    phase: Phase<T>,
    cooldown_until: Option<Instant>,
    /// When the most recent *forced* flight was dispatched (row 1: at
    /// dispatch, not completion). `None` until the first forced flight.
    last_forced_dispatch: Option<Instant>,
    /// Incremented every time a new flight is led. See the module doc
    /// comment's "stage 4d fix" section for why `run_flight` identifies
    /// its own flight by this counter rather than by `CacheKey` equality.
    flight_generation: u64,
}

impl<T> Slot<T> {
    fn new() -> Self {
        Self {
            entry: None,
            stored_at: None,
            phase: Phase::Idle,
            cooldown_until: None,
            last_forced_dispatch: None,
            flight_generation: 0,
        }
    }

    /// Leads a new flight for `key`: bumps the generation counter, installs
    /// a fresh watch channel as `phase`, and returns what the caller needs
    /// to run and later publish it.
    fn lead(&mut self, key: CacheKey) -> (watch::Sender<FlightSignal<T>>, u64) {
        self.flight_generation += 1;
        let generation = self.flight_generation;
        let (tx, rx) = watch::channel(None);
        self.phase = Phase::InFlight {
            rx,
            key,
            generation,
        };
        (tx, generation)
    }
}

/// Why [`FederationDocumentCache::lookup`] or `lookup_for_kid` refused to
/// return a document, or the leader's own fetch error (`Fetch`, carrying
/// exactly what the caller's closure returned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError<E> {
    /// Row 2/5: the slot's 30-second failure cooldown is active.
    Cooldown,
    /// Row 7: a forced flight was dispatched for this slot within the last
    /// 60 seconds; no network attempted.
    ForcedBudgetSpent,
    /// A flight this caller joined (row 3/6) failed. The leader's own
    /// caller gets `Fetch(E)` instead; only a joiner gets this.
    FlightFailed,
    /// The leader's fetch closure returned this error.
    Fetch(E),
    /// Row 6/8: a flight resolved (fresh, refetched, or 304-confirmed
    /// unchanged) but the requested `kid` is still absent. This is the
    /// caller's final answer for this call -- not retried.
    KidNotFound,
    /// `current_key()` returned `None`: there is no authoritative
    /// `(provider, version, activation generation)` to serve, join, or
    /// fetch for right now (the provider is disabled or absent). Checked
    /// before cooldown or flight state, since neither is meaningful
    /// without a live key.
    NoCurrentKey,
}

/// RFC 096 `:897-924`, the whole state table and its five easy-to-miss
/// rules, for one document kind (discovery or JWKS) across every
/// configured provider. See the module doc comment for the map-key and
/// clock design notes.
pub struct FederationDocumentCache<T> {
    slots: Mutex<HashMap<FederationProviderId, Slot<T>>>,
    clock: SharedMonotonicClock,
}

impl<T> FederationDocumentCache<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(clock: SharedMonotonicClock) -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            clock,
        }
    }

    /// Rows 1-4: no `kid` involved. Returns the fresh retained document
    /// without network (row 1); fails without network if the slot is
    /// cooling down (row 2); joins an in-flight fetch for the same key,
    /// ordinary or forced (row 3); or becomes the one flight leader for the
    /// current key (row 4). See the module doc comment's "stage 4d fix"
    /// section for the authority check this applies before every one of
    /// those four outcomes.
    pub async fn lookup<F, Fut, E>(
        &self,
        provider: FederationProviderId,
        current_key: impl Fn() -> Option<CacheKey> + Send,
        fetch: F,
    ) -> Result<(RetainedEntry<T>, CacheOutcome), LookupError<E>>
    where
        F: FnOnce(Option<RetainedEntry<T>>) -> Fut + Send,
        Fut: Future<Output = Result<(RetainedEntry<T>, CacheOutcome), E>> + Send,
        E: Send,
    {
        let now = self.clock.now();
        let decision = {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_insert_with(Slot::new);

            let live_key = current_key();
            if slot.entry.as_ref().map(RetainedEntry::key) != live_key {
                slot.entry = None;
                slot.stored_at = None;
            }
            let Some(live_key) = live_key else {
                return Err(LookupError::NoCurrentKey);
            };

            // Row 1, regardless of `phase`: a fresh, correctly-keyed entry
            // needs no network, even if some other (e.g. forced) flight
            // happens to be active for this slot right now.
            if let (Some(entry), Some(stored_at)) = (&slot.entry, slot.stored_at)
                && entry.is_fresh_after(now.saturating_duration_since(stored_at))
            {
                return Ok((entry.clone(), CacheOutcome::Retain));
            }

            match &slot.phase {
                Phase::InFlight { rx, key, .. } if *key == live_key => {
                    Decision::Join(rx.clone()) // row 3
                }
                _ => {
                    // Idle, or an active flight for a superseded key: row
                    // 4 applies either way, after the usual cooldown gate.
                    if let Some(until) = slot.cooldown_until
                        && now < until
                    {
                        return Err(LookupError::Cooldown); // row 2
                    }
                    let (tx, generation) = slot.lead(live_key);
                    Decision::Lead {
                        tx,
                        generation,
                        current_entry: slot.entry.clone(),
                    }
                }
            }
        };

        match decision {
            Decision::Join(rx) => Self::join(rx).await,
            Decision::Lead {
                tx,
                generation,
                current_entry,
            } => {
                self.run_flight(provider, tx, generation, current_entry, current_key, fetch)
                    .await
            }
        }
    }

    /// Rows 5-8: a `kid` not found in whatever document the caller already
    /// has. `has_kid` is run against whatever document the flight this
    /// call joins or leads resolves to -- a fresh fetch, a refetch, or a
    /// 304-confirmed-unchanged one all go through the identical check, so
    /// "a forced 304 proves the key set unchanged" needs no special case:
    /// the confirmed-unchanged document is examined exactly like a new one.
    pub async fn lookup_for_kid<F, Fut, E>(
        &self,
        provider: FederationProviderId,
        kid: &str,
        current_key: impl Fn() -> Option<CacheKey> + Send,
        has_kid: impl Fn(&T, &str) -> bool,
        fetch: F,
    ) -> Result<(RetainedEntry<T>, CacheOutcome), LookupError<E>>
    where
        F: FnOnce(Option<RetainedEntry<T>>) -> Fut + Send,
        Fut: Future<Output = Result<(RetainedEntry<T>, CacheOutcome), E>> + Send,
        E: Send,
    {
        let now = self.clock.now();
        let decision = {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_insert_with(Slot::new);

            let live_key = current_key();
            if slot.entry.as_ref().map(RetainedEntry::key) != live_key {
                slot.entry = None;
                slot.stored_at = None;
            }
            let Some(live_key) = live_key else {
                return Err(LookupError::NoCurrentKey);
            };

            let joinable = matches!(
                &slot.phase,
                Phase::InFlight { key, .. } if *key == live_key
            );
            if joinable {
                let Phase::InFlight { rx, .. } = &slot.phase else {
                    unreachable!("just matched Phase::InFlight above")
                };
                Decision::Join(rx.clone()) // row 6, same key
            } else {
                // Idle, or an active flight for a superseded key (which
                // row 6 does not join): rows 5/7/8 apply either way.
                if let Some(until) = slot.cooldown_until
                    && now < until
                {
                    // Row 5: do not touch `last_forced_dispatch` -- this
                    // failure spends no budget.
                    return Err(LookupError::Cooldown);
                }
                if let Some(dispatched) = slot.last_forced_dispatch
                    && now.saturating_duration_since(dispatched) < FORCED_REFRESH_WINDOW
                {
                    // Row 7: likewise untouched.
                    return Err(LookupError::ForcedBudgetSpent);
                }
                // Row 8: mark dispatch time *before* the fetch runs, so a
                // slow request cannot extend the 60-second window, and
                // bypass the freshness check entirely -- an unknown `kid`
                // means the currently-fresh document is exactly what is
                // wrong.
                slot.last_forced_dispatch = Some(now);
                let (tx, generation) = slot.lead(live_key);
                Decision::Lead {
                    tx,
                    generation,
                    current_entry: slot.entry.clone(),
                }
            }
        };

        let (entry, outcome) = match decision {
            Decision::Join(rx) => Self::join(rx).await?,
            Decision::Lead {
                tx,
                generation,
                current_entry,
            } => {
                self.run_flight(provider, tx, generation, current_entry, current_key, fetch)
                    .await?
            }
        };

        if has_kid(entry.document(), kid) {
            Ok((entry, outcome))
        } else {
            // Rows 6/8's "sole opportunity": no retry, no second flight.
            Err(LookupError::KidNotFound)
        }
    }

    /// Awaits the fetch, tells any joiners, then re-acquires the lock to
    /// publish or cool down -- but only if this flight is still the one
    /// the slot's `phase` points at. RFC 096 `:897-924` rule 4: a result is
    /// published only if `current_key()` still names the same
    /// `(provider, version, activation generation)` the fetch itself
    /// produced -- a disable or version change while the flight was in the
    /// air discards it rather than retaining a document for an activation
    /// that is no longer current. See the module doc comment's "stage 4d
    /// fix" section for why a *superseded* flight (one a newer caller has
    /// already led a replacement for) must not touch `phase`,
    /// `cooldown_until`, or `entry` at all, success or failure -- and why
    /// that is identified by `generation` rather than by `CacheKey`.
    /// Joiners of *this* flight still receive its fetched data for their
    /// one waiting operation regardless of whether it ends up superseded
    /// or discarded; only the slot's own retained state is affected.
    async fn run_flight<F, Fut, E>(
        &self,
        provider: FederationProviderId,
        tx: watch::Sender<FlightSignal<T>>,
        generation: u64,
        current_entry: Option<RetainedEntry<T>>,
        current_key: impl Fn() -> Option<CacheKey>,
        fetch: F,
    ) -> Result<(RetainedEntry<T>, CacheOutcome), LookupError<E>>
    where
        F: FnOnce(Option<RetainedEntry<T>>) -> Fut,
        Fut: Future<Output = Result<(RetainedEntry<T>, CacheOutcome), E>>,
    {
        let result = fetch(current_entry).await;

        let signal: FlightSignal<T> = Some(match &result {
            Ok((entry, outcome)) => Ok((entry.clone(), *outcome)),
            Err(_) => Err(()),
        });
        // No receivers is fine -- nobody was waiting.
        let _ = tx.send(signal);

        let published_at = self.clock.now();
        {
            let mut slots = lock(&self.slots);
            // Re-using `entry`/`or_insert_with` rather than `get_mut` means
            // there is no "this slot must already exist" to assert at all
            // -- true in practice (we created it above), but stating it as
            // a lookup that cannot fail is stronger than asserting it would
            // be.
            let slot = slots.entry(provider).or_insert_with(Slot::new);
            let still_mine = matches!(
                &slot.phase,
                Phase::InFlight { generation: g, .. } if *g == generation
            );
            if still_mine {
                slot.phase = Phase::Idle;
                match &result {
                    Ok((entry, _outcome)) => {
                        if current_key() == Some(entry.key()) {
                            slot.entry = Some(entry.clone());
                            slot.stored_at = Some(published_at);
                        }
                        // A success always clears a cooldown, even a
                        // discarded one: the network is reachable and
                        // answering.
                        slot.cooldown_until = None;
                    }
                    Err(_) => {
                        slot.cooldown_until = Some(published_at + FAILURE_COOLDOWN);
                    }
                }
            }
            // else: a newer flight already owns this slot. This result,
            // success or failure, is not this module's concern any more --
            // the caller who led it still gets it back below.
        }

        result.map_err(LookupError::Fetch)
    }

    async fn join<E>(
        mut rx: watch::Receiver<FlightSignal<T>>,
    ) -> Result<(RetainedEntry<T>, CacheOutcome), LookupError<E>> {
        match rx.wait_for(Option::is_some).await {
            // `wait_for`'s predicate guarantees `Some`, but matching rather
            // than asserting it means a future change to that predicate
            // fails safe (a generic `FlightFailed`) instead of panicking.
            Ok(value) => match value.clone() {
                Some(result) => result.map_err(|()| LookupError::FlightFailed),
                None => Err(LookupError::FlightFailed),
            },
            Err(_) => Err(LookupError::FlightFailed),
        }
    }
}

enum Decision<T> {
    Join(watch::Receiver<FlightSignal<T>>),
    Lead {
        tx: watch::Sender<FlightSignal<T>>,
        generation: u64,
        current_entry: Option<RetainedEntry<T>>,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "federation_cache/tests.rs"]
mod tests;
