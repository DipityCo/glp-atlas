//! Everything Atlas keeps: the dose log, the medication being tracked, and the device storage
//! behind both.
//!
//! The two are held in signals and written back to the device as one document after every
//! change, by [`use_store`], in the shapes [`crate::stored`] freezes. Nothing leaves the device.

use std::cmp::{Ordering, Reverse};

use chrono::{Days, Local, NaiveDate, NaiveTime, Weekday};
use dioxus::document;
use dioxus::prelude::*;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::formulary::Drug;
use crate::stock::{Stock, StockDraft, StockId, Vial, VialId};
use crate::stored::{Reading, Writing, VERSION};
use crate::units::Clock;

/// Days between doses when no drug has been chosen.
const DEFAULT_INTERVAL_DAYS: u32 = 7;

/// Reads the stored document, reporting separately that local storage could not be reached at
/// all. A `WebView` with storage disabled throws rather than returning nothing.
const READ: &str = r#"
try {
  dioxus.send({ ok: true, raw: localStorage.getItem("atlas.v1") });
} catch (error) {
  dioxus.send({ ok: false, raw: null });
}
"#;

/// Writes documents back, one at a time, for as long as the app runs. The payload arrives over
/// the channel rather than baked into this source, so a note carrying quotes or newlines cannot
/// end up as script.
const WRITE: &str = r#"
while (true) {
  let raw;
  try {
    raw = await dioxus.recv();
  } catch (error) {
    break;
  }
  try {
    localStorage.setItem("atlas.v1", raw);
  } catch (error) {}
}
"#;

/// Where the records stand with the device's storage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Nothing is written back yet.
    Loading,
    /// Every change from here is written back.
    Ready,
    /// Could not be read, or was not in a shape this build understands. Changes last for this
    /// session only; nothing is written back, so what is stored survives.
    Unreadable,
}

/// Names a dose for as long as the app runs. Sub-pages hold one of these rather than a position
/// in the log, so a record deleted from under an open page is a miss and never a different dose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DoseId(pub(crate) u64);

impl DoseId {
    /// Only the log mints these, so an id always names a dose that was really logged. The tests
    /// need one without a log to hand.
    #[cfg(test)]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Site {
    LeftAbdomen,
    RightAbdomen,
    LeftThigh,
    RightThigh,
}

impl Site {
    pub const ALL: [Site; 4] = [
        Site::LeftAbdomen,
        Site::RightAbdomen,
        Site::LeftThigh,
        Site::RightThigh,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Site::LeftAbdomen => "Left abdomen",
            Site::RightAbdomen => "Right abdomen",
            Site::LeftThigh => "Left thigh",
            Site::RightThigh => "Right thigh",
        }
    }
}

/// Where a dose with no recorded time is placed. Midday is never more than half a day out.
pub const ASSUMED_HOUR: u32 = 12;

/// What one vial gave to one dose. Entered as a `DrawnDraft`, which holds the same share as raw
/// text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Drawn {
    pub vial: VialId,
    /// The shares of a dose sum to it only where the whole of it came off the shelf.
    pub micrograms: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Dose {
    pub id: DoseId,
    pub taken: NaiveDate,
    /// The time of day it was given, where that is known. Left optional because a dose entered
    /// from memory has a date and no hour, and inventing one would put a figure in the record the
    /// user never gave. The level model places a dose without one at [`ASSUMED_HOUR`].
    pub time: Option<NaiveTime>,
    /// Which drug this injection was, per dose rather than per person: a log spans a switch from
    /// one drug to another, and each clears at its own rate.
    ///
    /// Unset where the drug was never chosen. Such a dose stays in the log and out of every
    /// curve, since nothing can be modelled without knowing the molecule.
    pub drug: Option<Drug>,
    /// Strength in micrograms. Whole numbers, because doses are compared and differenced, and a
    /// tenth of a milligram is a real difference that a binary fraction does not hold exactly.
    pub micrograms: u32,
    pub site: Site,
    /// In the order they were drawn. Empty for a dose that did not come off the shelf.
    pub from: Vec<Drawn>,
    pub note: String,
}

impl Dose {
    /// Falls back to [`ASSUMED_HOUR`] where none was recorded.
    pub fn hour(&self) -> NaiveTime {
        self.time.unwrap_or_else(assumed_time)
    }
}

/// The fields a dose is entered with. The log owns identity, ordering and storage.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Draft {
    pub taken: NaiveDate,
    pub time: Option<NaiveTime>,
    pub drug: Option<Drug>,
    pub micrograms: u32,
    pub site: Site,
    pub from: Vec<Drawn>,
    pub note: String,
}

pub fn assumed_time() -> NaiveTime {
    NaiveTime::from_hms_opt(ASSUMED_HOUR, 0, 0).expect("midday is a time of day")
}

/// The time of day, where the device is.
pub fn time_of_day() -> NaiveTime {
    Local::now().time()
}

/// One rung of a titration plan: a strength, held for a number of weeks.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TitrationStep {
    pub micrograms: u32,
    pub weeks: u32,
}

/// The medication being tracked, as the user records it.
///
/// Atlas states this record back and never proposes any of it: a drug, a dose day and a
/// titration plan come from a prescriber.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Medication {
    /// Unset until chosen. Half-life varies severalfold across [`Drug::ALL`], so a guess here
    /// would put the level model out by more than it would ever be worth.
    pub drug: Option<Drug>,
    /// The day the dose is prescribed for. Recorded and shown, but it does not move the next dose
    /// along: that follows the last dose by [`Medication::interval_days`], since pulling a dose
    /// that slipped back onto its old day is a prescriber's decision.
    pub dose_day: Option<Weekday>,
    /// Day zero of [`Medication::titration`]. Falls back to the first dose in the log.
    pub started: Option<NaiveDate>,
    pub titration: Vec<TitrationStep>,
}

impl Medication {
    pub fn interval_days(&self) -> u32 {
        self.drug.map_or(DEFAULT_INTERVAL_DAYS, Drug::interval_days)
    }
}

/// Where a date falls in a titration plan. Positions count from one, as the plan reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rung {
    pub step: usize,
    pub steps: usize,
    pub micrograms: u32,
    pub week: u32,
    pub weeks: u32,
}

/// Where `today` falls in a plan begun on `started`, or `None` once the plan has run out.
pub fn rung(titration: &[TitrationStep], started: NaiveDate, today: NaiveDate) -> Option<Rung> {
    let mut week = u32::try_from((today - started).num_days().max(0) / 7).ok()?;
    for (index, step) in titration.iter().enumerate() {
        if week < step.weeks {
            return Some(Rung {
                step: index + 1,
                steps: titration.len(),
                micrograms: step.micrograms,
                week: week + 1,
                weeks: step.weeks,
            });
        }
        week -= step.weeks;
    }
    None
}

/// What [`READ`] reports back.
#[derive(Deserialize)]
struct Fetched {
    ok: bool,
    raw: Option<String>,
}

/// Today, where the device is.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The cycle running from a dose to the next one due.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cycle {
    pub due: NaiveDate,
    /// Positive before the dose is due, negative once past it.
    pub remaining: i64,
    interval: i64,
}

impl Cycle {
    /// `None` for a date so far out that the next dose has no place on the calendar.
    pub fn new(last: NaiveDate, today: NaiveDate, interval_days: u32) -> Option<Self> {
        let due = last.checked_add_days(Days::new(u64::from(interval_days)))?;
        Some(Self {
            due,
            remaining: (due - today).num_days(),
            interval: i64::from(interval_days.max(1)),
        })
    }

    /// Share of the cycle behind us, 0–100, filling the ring.
    pub fn fill(self) -> u32 {
        let elapsed = (self.interval - self.remaining).clamp(0, self.interval);
        u32::try_from(elapsed * 100 / self.interval).unwrap_or(100)
    }

    /// The figure in the ring and the word under it.
    pub fn ring(self) -> (i64, &'static str) {
        match self.remaining {
            1 => (1, "day"),
            days if days < 0 => (-days, "late"),
            days => (days, "days"),
        }
    }
}

/// How recently a site was used: 0 for the last dose, larger the longer ago, [`usize::MAX`] if
/// it has never been used. `recent` is newest first.
fn age(site: Site, recent: &[Dose]) -> usize {
    recent
        .iter()
        .position(|dose| dose.site == site)
        .unwrap_or(usize::MAX)
}

/// The site to offer next: the one longest since used, so logging in a row rotates through all
/// four. Ties, including the first dose ever logged, take the first in [`Site::ALL`].
pub fn suggest_site(recent: &[Dose]) -> Site {
    Site::ALL
        .into_iter()
        .min_by_key(|site| Reverse(age(*site, recent)))
        .unwrap_or(Site::LeftAbdomen)
}

/// Newest first, by the hour where there is one, and within the same moment the most recently
/// entered first.
fn newest_first(a: &Dose, b: &Dose) -> Ordering {
    b.taken
        .cmp(&a.taken)
        .then_with(|| b.hour().cmp(&a.hour()))
        .then(b.id.0.cmp(&a.id.0))
}

/// The records, shared through context.
///
/// Every field is a [`Signal`], a copyable handle rather than copied state, so all copies reach
/// the same slots and handlers can take a `Store` by value. The mutations do not write to the
/// device; [`use_store`] does that from an effect.
#[derive(Clone, Copy)]
pub struct Store {
    /// The log, kept newest first.
    doses: Signal<Vec<Dose>>,
    stock: Signal<Vec<Stock>>,
    medication: Signal<Medication>,
    /// The face every time of day in the app is written on.
    clock: Signal<Clock>,
    status: Signal<Status>,
    /// Never reused within a session, so an id held by an open page cannot come to name a dose
    /// logged after it. Not stored: [`Store::install`] sets it past everything read back.
    next_id: Signal<u64>,
    next_stock_id: Signal<u64>,
    next_vial_id: Signal<u64>,
}

fn next_after(ids: impl Iterator<Item = u64>) -> u64 {
    ids.max().map_or(0, |highest| highest.saturating_add(1))
}

impl Store {
    pub fn new() -> Self {
        Self {
            doses: Signal::new(Vec::new()),
            stock: Signal::new(Vec::new()),
            medication: Signal::new(Medication::default()),
            clock: Signal::new(Clock::default()),
            status: Signal::new(Status::Loading),
            next_id: Signal::new(0),
            next_stock_id: Signal::new(0),
            next_vial_id: Signal::new(0),
        }
    }

    pub fn status(self) -> Status {
        *self.status.read()
    }

    /// Newest first.
    pub fn all(self) -> Vec<Dose> {
        self.doses.read().clone()
    }

    pub fn get(self, id: DoseId) -> Option<Dose> {
        self.doses.read().iter().find(|dose| dose.id == id).cloned()
    }

    pub fn previous(self, id: DoseId) -> Option<Dose> {
        let doses = self.doses.read();
        let position = doses.iter().position(|dose| dose.id == id)?;
        doses.get(position + 1).cloned()
    }

    /// Counting from the first dose ever logged.
    pub fn number(self, id: DoseId) -> Option<usize> {
        let doses = self.doses.read();
        let position = doses.iter().position(|dose| dose.id == id)?;
        Some(doses.len() - position)
    }

    pub fn add(&mut self, draft: Draft) -> DoseId {
        let id = DoseId(*self.next_id.read());
        self.next_id.set(id.0.saturating_add(1));
        let mut doses = self.doses.write();
        doses.push(Dose {
            id,
            taken: draft.taken,
            time: draft.time,
            drug: draft.drug,
            micrograms: draft.micrograms,
            site: draft.site,
            from: draft.from,
            note: draft.note,
        });
        doses.sort_by(newest_first);
        id
    }

    /// A dose that is no longer in the log is left alone.
    pub fn update(&mut self, id: DoseId, draft: Draft) {
        let mut doses = self.doses.write();
        let Some(dose) = doses.iter_mut().find(|dose| dose.id == id) else {
            return;
        };
        dose.taken = draft.taken;
        dose.time = draft.time;
        dose.drug = draft.drug;
        dose.micrograms = draft.micrograms;
        dose.site = draft.site;
        dose.from = draft.from;
        dose.note = draft.note;
        doses.sort_by(newest_first);
    }

    pub fn remove(&mut self, id: DoseId) {
        self.doses.write().retain(|dose| dose.id != id);
    }

    pub fn stock(self) -> Vec<Stock> {
        self.stock.read().clone()
    }

    pub fn stock_of(self, id: StockId) -> Option<Stock> {
        self.stock
            .read()
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
    }

    pub fn add_stock(&mut self, draft: StockDraft) -> StockId {
        if let Some(id) = self.count_in(&draft) {
            return id;
        }
        let id = StockId(*self.next_stock_id.read());
        self.next_stock_id.set(id.0.saturating_add(1));
        self.stock.write().insert(
            0,
            Stock {
                id,
                drug: draft.drug,
                label: draft.label,
                micrograms: draft.micrograms,
                form: draft.form,
                sealed: draft.sealed,
                open: Vec::new(),
                note: draft.note,
            },
        );
        id
    }

    fn count_in(&mut self, draft: &StockDraft) -> Option<StockId> {
        let mut stock = self.stock.write();
        let entry = stock.iter_mut().find(|entry| entry.describes(draft))?;
        entry.sealed = entry.sealed.saturating_add(draft.sealed);
        if entry.note.trim().is_empty() {
            entry.note.clone_from(&draft.note);
        }
        Some(entry.id)
    }

    /// Never merges, unlike [`Store::add_stock`]: an entry edited into another's description
    /// stands beside it, since open pages hold the ids of the vials in use.
    pub fn update_stock(&mut self, id: StockId, draft: StockDraft) {
        let mut stock = self.stock.write();
        let Some(entry) = stock.iter_mut().find(|entry| entry.id == id) else {
            return;
        };
        entry.drug = draft.drug;
        entry.label = draft.label;
        entry.micrograms = draft.micrograms;
        entry.form = draft.form;
        entry.sealed = draft.sealed;
        entry.note = draft.note;
    }

    pub fn open_vial(&mut self, id: StockId, microlitres: u32, on: NaiveDate) -> Option<VialId> {
        let vial = VialId(*self.next_vial_id.read());
        {
            let mut stock = self.stock.write();
            let entry = stock.iter_mut().find(|entry| entry.id == id)?;
            if entry.sealed == 0 {
                return None;
            }
            entry.sealed -= 1;
            entry.open.push(Vial {
                id: vial,
                micrograms: entry.micrograms,
                microlitres,
                opened: on,
            });
        }
        self.next_vial_id.set(vial.0.saturating_add(1));
        Some(vial)
    }

    pub fn discard_vial(&mut self, id: StockId, vial: VialId) {
        let mut discarded = false;
        if let Some(entry) = self.stock.write().iter_mut().find(|entry| entry.id == id) {
            let held = entry.open.len();
            entry.open.retain(|open| open.id != vial);
            discarded = entry.open.len() != held;
        }
        if discarded {
            self.release(&[vial]);
        }
    }

    pub fn remove_stock(&mut self, id: StockId) {
        let mut gone = Vec::new();
        self.stock.write().retain(|entry| {
            if entry.id != id {
                return true;
            }
            gone.extend(entry.open.iter().map(|vial| vial.id));
            false
        });
        self.release(&gone);
    }

    fn release(&mut self, vials: &[VialId]) {
        for dose in self.doses.write().iter_mut() {
            dose.from.retain(|drawn| !vials.contains(&drawn.vial));
        }
    }

    pub fn open_vials(self) -> Vec<(Stock, Vial)> {
        self.stock
            .read()
            .iter()
            .flat_map(|entry| entry.open.iter().map(|vial| (entry.clone(), vial.clone())))
            .collect()
    }

    pub fn vial(self, id: VialId) -> Option<(Stock, Vial)> {
        self.stock.read().iter().find_map(|entry| {
            let vial = entry.open.iter().find(|open| open.id == id)?;
            Some((entry.clone(), vial.clone()))
        })
    }

    pub fn remaining(self, vial: &Vial) -> u32 {
        let drawn = self
            .doses
            .read()
            .iter()
            .flat_map(|dose| dose.from.iter())
            .filter(|drawn| drawn.vial == vial.id)
            .fold(0_u32, |sum, drawn| sum.saturating_add(drawn.micrograms));
        vial.micrograms.saturating_sub(drawn)
    }

    pub fn held(self, entry: &Stock) -> u32 {
        entry.open.iter().fold(
            entry.sealed.saturating_mul(entry.micrograms),
            |sum, vial| sum.saturating_add(self.remaining(vial)),
        )
    }

    pub fn supply(self) -> (u32, u32) {
        self.stock
            .read()
            .iter()
            .fold((0, 0), |(sealed, open), entry| {
                (
                    sealed.saturating_add(entry.sealed),
                    open.saturating_add(u32::try_from(entry.open.len()).unwrap_or(u32::MAX)),
                )
            })
    }

    pub fn medication(self) -> Medication {
        self.medication.read().clone()
    }

    pub fn set_medication(&mut self, medication: Medication) {
        self.medication.set(medication);
    }

    pub fn clock(self) -> Clock {
        *self.clock.read()
    }

    pub fn set_clock(&mut self, clock: Clock) {
        self.clock.set(clock);
    }

    /// Deletes the log, the shelf, the medication record and the clock setting.
    pub fn wipe(&mut self) {
        self.doses.set(Vec::new());
        self.stock.set(Vec::new());
        self.medication.set(Medication::default());
        self.clock.set(Clock::default());
        // Even from `Unreadable`: a document that could not be read is still deleted on request.
        self.status.set(Status::Ready);
    }

    /// Once, at startup.
    async fn hydrate(mut self) {
        let mut eval = document::eval(READ);
        let fetched = eval.recv::<Fetched>().await;
        // A wipe while the read was in flight has already settled the records; installing the
        // document it deleted would undo it.
        if self.status() != Status::Loading {
            return;
        }
        let status = match fetched {
            Ok(Fetched {
                ok: true,
                raw: None,
            }) => Status::Ready,
            Ok(Fetched {
                ok: true,
                raw: Some(raw),
            }) => match serde_json::from_str::<Reading>(&raw) {
                Ok(reading) if reading.version == VERSION => {
                    let doses = reading.doses.into_iter().map(Dose::from).collect();
                    let stock = reading.stock.into_iter().map(Stock::from).collect();
                    self.install(doses, stock);
                    self.medication.set(reading.medication);
                    self.clock.set(reading.clock);
                    Status::Ready
                }
                _ => Status::Unreadable,
            },
            _ => Status::Unreadable,
        };
        self.status.set(status);
    }

    /// Puts records read from the device in place, leaving every id counter past everything in
    /// them.
    fn install(&mut self, mut doses: Vec<Dose>, stock: Vec<Stock>) {
        doses.sort_by(newest_first);
        self.next_id
            .set(next_after(doses.iter().map(|dose| dose.id.0)));
        self.next_stock_id
            .set(next_after(stock.iter().map(|entry| entry.id.0)));
        self.next_vial_id.set(next_after(
            stock
                .iter()
                .flat_map(|entry| entry.open.iter())
                .map(|vial| vial.id.0),
        ));
        self.doses.set(doses);
        self.stock.set(stock);
    }

    /// The document to write back, or `None` while there is nothing to put over the stored one:
    /// an empty starting log must not land on top of one, and a log this build cannot parse must
    /// not be replaced by one it can.
    fn document(self) -> Option<String> {
        if self.status() != Status::Ready {
            return None;
        }
        let doses = self.doses.read();
        let medication = self.medication.read();
        serde_json::to_string(&Writing {
            version: VERSION,
            doses: doses.iter().map(Into::into).collect(),
            stock: self.stock.read().iter().map(Into::into).collect(),
            medication: &medication,
            clock: *self.clock.read(),
        })
        .ok()
    }

    fn persist(self, writer: Coroutine<String>) {
        if let Some(document) = self.document() {
            writer.send(document);
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

/// Hydrates the log from the device, then writes it back after every change. Call once, above
/// everything that reads it.
pub fn use_store() -> Store {
    let store = use_context_provider(Store::new);

    // One evaluator, fed in order, for the life of the app. An evaluator per write is a channel
    // per write, and two of them can resolve out of order, which lets an older document land
    // last: hold a key down in the titration form and the record can end a keystroke behind.
    let writer = use_coroutine(|mut documents: UnboundedReceiver<String>| async move {
        let eval = document::eval(WRITE);
        while let Some(document) = documents.next().await {
            eval.send(document).ok();
        }
    });

    use_hook(move || spawn(store.hydrate()));
    // Reads the log, so it runs again whenever the log changes.
    use_effect(move || store.persist(writer));
    store
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stock::Form;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date the test wrote")
    }

    fn time(text: &str) -> NaiveTime {
        NaiveTime::parse_from_str(text, "%H:%M").expect("a time the test wrote")
    }

    fn draft(taken: &str, micrograms: u32, site: Site) -> Draft {
        Draft {
            taken: date(taken),
            time: None,
            drug: None,
            micrograms,
            site,
            from: Vec::new(),
            note: String::new(),
        }
    }

    // Signals need a runtime and an owning scope; `Store` is otherwise plain state.
    fn store(f: impl FnOnce(Store)) {
        let dom = VirtualDom::new(|| rsx! { div {} });
        dom.in_runtime(|| {
            dioxus::core::Runtime::current().in_scope(ScopeId::ROOT, || f(Store::new()));
        });
    }

    #[test]
    fn a_new_log_is_empty_and_still_loading() {
        store(|store| {
            assert_eq!(store.status(), Status::Loading);
            assert!(store.all().is_empty());
        });
    }

    #[test]
    fn the_clock_opens_on_the_face_the_app_has_always_worn() {
        store(|mut store| {
            assert_eq!(store.clock(), Clock::TwentyFourHour);

            store.set_clock(Clock::TwelveHour);
            assert_eq!(store.clock(), Clock::TwelveHour);
        });
    }

    #[test]
    fn a_wipe_leaves_nothing_of_the_records_or_the_settings() {
        store(|mut store| {
            store.add(draft("2026-08-06", 2500, Site::LeftAbdomen));
            store.add_stock(vials(30_000, 4));
            store.set_medication(Medication {
                drug: Some(Drug::Semaglutide),
                ..Medication::default()
            });
            store.set_clock(Clock::TwelveHour);

            store.wipe();

            assert!(store.all().is_empty());
            assert!(store.stock().is_empty());
            assert_eq!(store.medication(), Medication::default());
            assert_eq!(store.clock(), Clock::default());
        });
    }

    #[test]
    fn nothing_is_written_back_before_the_stored_document_has_been_read() {
        store(|store| {
            assert_eq!(store.document(), None);
        });
    }

    /// The document a wipe leaves behind is the one that lands on the device, so what it carries
    /// is what survives a restart.
    #[test]
    fn a_wipe_writes_back_a_document_holding_nothing_even_over_an_unreadable_one() {
        store(|mut store| {
            store.add(draft("2026-08-06", 2500, Site::LeftAbdomen));
            store.add_stock(vials(30_000, 4));
            store.set_clock(Clock::TwelveHour);
            store.status.set(Status::Unreadable);

            store.wipe();

            let document = store.document().expect("a wipe is written back");
            let read: Reading = serde_json::from_str(&document).expect("and reads back");
            assert_eq!(read.version, VERSION);
            assert!(read.doses.is_empty());
            assert!(read.stock.is_empty());
            assert_eq!(read.medication, Medication::default());
            assert_eq!(read.clock, Clock::default());
        });
    }

    #[test]
    fn an_id_minted_before_a_wipe_names_no_dose_logged_after_one() {
        store(|mut store| {
            let gone = store.add(draft("2026-08-06", 2500, Site::LeftAbdomen));

            store.wipe();
            let logged = store.add(draft("2026-08-13", 2500, Site::LeftThigh));

            assert_ne!(logged, gone);
            assert_eq!(store.get(gone), None);
        });
    }

    #[test]
    fn doses_come_back_newest_first_however_they_were_entered() {
        store(|mut store| {
            store.add(draft("2026-07-30", 2500, Site::LeftThigh));
            let newest = store.add(draft("2026-08-06", 2500, Site::LeftAbdomen));
            store.add(draft("2026-07-23", 1700, Site::RightThigh));

            let dates: Vec<_> = store.all().iter().map(|dose| dose.taken).collect();
            assert_eq!(
                dates,
                vec![date("2026-08-06"), date("2026-07-30"), date("2026-07-23")]
            );
            assert_eq!(store.all().first().map(|dose| dose.id), Some(newest));
        });
    }

    #[test]
    fn a_dose_keeps_the_hour_it_was_given_at_and_reports_it_back() {
        store(|mut store| {
            let id = store.add(Draft {
                taken: date("2026-08-06"),
                time: Some(time("07:30")),
                drug: None,
                micrograms: 2500,
                site: Site::LeftThigh,
                from: Vec::new(),
                note: String::new(),
            });

            let dose = store.get(id).expect("the dose is logged");
            assert_eq!(dose.time, Some(time("07:30")));
            assert_eq!(dose.hour(), time("07:30"));
        });
    }

    #[test]
    fn a_dose_with_no_hour_is_taken_to_have_been_given_at_midday() {
        store(|mut store| {
            let id = store.add(draft("2026-08-06", 2500, Site::LeftThigh));
            let dose = store.get(id).expect("the dose is logged");

            assert_eq!(dose.time, None, "nothing is invented into the record");
            assert_eq!(dose.hour(), assumed_time());
            assert_eq!(dose.hour(), time("12:00"));
        });
    }

    #[test]
    fn doses_on_one_day_come_back_in_the_order_they_were_given() {
        store(|mut store| {
            let morning = store.add(Draft {
                taken: date("2026-08-06"),
                time: Some(time("07:30")),
                drug: None,
                micrograms: 2500,
                site: Site::LeftThigh,
                from: Vec::new(),
                note: String::new(),
            });
            let evening = store.add(Draft {
                taken: date("2026-08-06"),
                time: Some(time("21:00")),
                drug: None,
                micrograms: 2500,
                site: Site::RightThigh,
                from: Vec::new(),
                note: String::new(),
            });

            let order: Vec<_> = store.all().iter().map(|dose| dose.id).collect();
            assert_eq!(order, vec![evening, morning], "the later hour leads");
        });
    }

    /// The dose in `STORED_DOCUMENT`, drawn wholly from the vial on the shelf in `shelved`.
    fn logged() -> Dose {
        Dose {
            id: DoseId::new(3),
            taken: date("2026-08-06"),
            time: Some(time("07:30")),
            drug: Some(Drug::Tirzepatide),
            micrograms: 2500,
            site: Site::LeftThigh,
            from: vec![Drawn {
                vial: VialId::new(4),
                micrograms: 2500,
            }],
            note: "From the fridge".to_owned(),
        }
    }

    #[test]
    fn two_doses_on_one_day_put_the_later_entry_first() {
        store(|mut store| {
            let first = store.add(draft("2026-08-06", 2500, Site::LeftThigh));
            let second = store.add(draft("2026-08-06", 2500, Site::RightThigh));

            let order: Vec<_> = store.all().iter().map(|dose| dose.id).collect();
            assert_eq!(order, vec![second, first]);
        });
    }

    #[test]
    fn a_dose_is_reached_by_its_id_and_keeps_it_across_a_change_of_date() {
        store(|mut store| {
            store.add(draft("2026-08-06", 2500, Site::LeftThigh));
            let id = store.add(draft("2026-07-30", 1700, Site::RightThigh));

            store.update(id, draft("2026-08-20", 2500, Site::LeftAbdomen));
            let moved = store.get(id).expect("the dose is still logged");
            assert_eq!(moved.taken, date("2026-08-20"));
            assert_eq!(moved.site, Site::LeftAbdomen);
            assert_eq!(
                store.all().first().map(|dose| dose.id),
                Some(id),
                "the later date moves it to the front"
            );
        });
    }

    #[test]
    fn editing_a_dose_that_is_gone_changes_nothing() {
        store(|mut store| {
            let id = store.add(draft("2026-08-06", 2500, Site::LeftThigh));
            store.remove(id);

            store.update(id, draft("2026-08-07", 5000, Site::RightThigh));
            assert!(store.all().is_empty());
            assert_eq!(store.get(id), None);
        });
    }

    #[test]
    fn an_id_is_never_handed_out_twice() {
        store(|mut store| {
            let first = store.add(draft("2026-08-06", 2500, Site::LeftThigh));
            store.remove(first);
            let second = store.add(draft("2026-08-06", 2500, Site::LeftThigh));

            assert_ne!(first, second, "a deleted dose does not lend out its id");
            assert_eq!(store.get(first), None);
        });
    }

    #[test]
    fn a_dose_knows_the_one_before_it_and_its_place_in_the_log() {
        store(|mut store| {
            let first = store.add(draft("2026-07-23", 1700, Site::LeftThigh));
            let second = store.add(draft("2026-07-30", 2500, Site::RightThigh));

            assert_eq!(store.previous(second).map(|dose| dose.id), Some(first));
            assert_eq!(store.previous(first), None, "nothing precedes the first");
            assert_eq!(store.number(first), Some(1));
            assert_eq!(store.number(second), Some(2));
        });
    }

    #[test]
    fn a_document_read_back_from_storage_is_the_one_that_was_written() {
        store(|mut store| {
            store.add(Draft {
                taken: date("2026-08-06"),
                time: Some(time("08:15")),
                drug: None,
                micrograms: 2500,
                site: Site::LeftAbdomen,
                from: Vec::new(),
                note: "Quotes \" and a\nnewline".to_owned(),
            });
            store.add(draft("2026-07-30", 1700, Site::RightThigh));
            store.set_medication(Medication {
                drug: Some(Drug::Semaglutide),
                dose_day: Some(Weekday::Thu),
                started: Some(date("2026-04-02")),
                titration: vec![
                    TitrationStep {
                        micrograms: 250,
                        weeks: 4,
                    },
                    TitrationStep {
                        micrograms: 2500,
                        weeks: 4,
                    },
                ],
            });
            store.set_clock(Clock::TwelveHour);
            store.status.set(Status::Ready);
            let doses = store.all();
            let medication = store.medication();

            let payload = serde_json::to_string(&Writing {
                version: VERSION,
                doses: doses.iter().map(Into::into).collect(),
                stock: Vec::new(),
                medication: &medication,
                clock: store.clock(),
            })
            .expect("the document serialises");
            let read: Reading = serde_json::from_str(&payload).expect("and reads back");

            assert_eq!(read.version, VERSION);
            assert_eq!(
                read.doses.into_iter().map(Dose::from).collect::<Vec<_>>(),
                doses
            );
            assert_eq!(read.medication, medication);
            assert_eq!(read.clock, Clock::TwelveHour);
        });
    }

    fn shelved() -> Stock {
        Stock {
            id: StockId::new(1),
            drug: Some(Drug::Tirzepatide),
            label: String::new(),
            micrograms: 30_000,
            form: Form::Lyophilized,
            sealed: 9,
            open: vec![Vial {
                id: VialId::new(4),
                micrograms: 30_000,
                microlitres: 2000,
                opened: date("2026-08-12"),
            }],
            note: String::new(),
        }
    }

    /// A whole document, holding the dose in `logged` drawn from the vial on the shelf in
    /// `shelved`. Pins the envelope around the records: the names they are stored under and the
    /// order they are written in, which no single record's frozen shape can.
    const STORED_DOCUMENT: &str = concat!(
        r#"{"version":1,"doses":[{"id":3,"taken":"2026-08-06","time":"07:30:00","#,
        r#""drug":"Tirzepatide","micrograms":2500,"site":"LeftThigh","#,
        r#""from":[{"vial":4,"micrograms":2500}],"#,
        r#""note":"From the fridge"}],"#,
        r#""stock":[{"id":1,"drug":"Tirzepatide","label":"","micrograms":30000,"#,
        r#""form":"Lyophilized","sealed":9,"#,
        r#""open":[{"id":4,"micrograms":30000,"microlitres":2000,"opened":"2026-08-12"}],"#,
        r#""note":""}],"#,
        r#""medication":{"drug":"Semaglutide","dose_day":"Thu","started":"2026-04-02","#,
        r#""titration":[{"micrograms":250,"weeks":4}]},"#,
        r#""clock":"TwelveHour"}"#
    );

    fn prescribed() -> Medication {
        Medication {
            drug: Some(Drug::Semaglutide),
            dose_day: Some(Weekday::Thu),
            started: Some(date("2026-04-02")),
            titration: vec![TitrationStep {
                micrograms: 250,
                weeks: 4,
            }],
        }
    }

    /// This failing means the document written to the device has changed, whichever record moved.
    /// Nothing is released, so that is allowed; read the diff and make sure it was meant.
    #[test]
    fn the_stored_document_holds_the_shape_it_was_written_in() {
        let written = serde_json::to_string(&Writing {
            version: VERSION,
            doses: vec![(&logged()).into()],
            stock: vec![(&shelved()).into()],
            medication: &prescribed(),
            clock: Clock::TwelveHour,
        })
        .expect("the document serialises");
        assert_eq!(written, STORED_DOCUMENT);
    }

    /// The read path end to end, which the record tests stop short of: a document off the device
    /// through [`Reading`] and into a live log.
    #[test]
    fn a_document_from_the_device_lands_in_the_log_with_its_vials_still_attached() {
        store(|mut store| {
            let read: Reading = serde_json::from_str(STORED_DOCUMENT).expect("it reads");
            assert_eq!(read.version, VERSION);

            store.install(
                read.doses.into_iter().map(Dose::from).collect(),
                read.stock.into_iter().map(Stock::from).collect(),
            );

            assert_eq!(store.all(), vec![logged()]);
            assert_eq!(store.stock(), vec![shelved()]);

            let (entry, vial) = store.vial(VialId::new(4)).expect("the vial it came out of");
            assert_eq!(entry.id, StockId::new(1));
            assert_eq!(
                store.remaining(&vial),
                27_500,
                "the stored dose is drawn out of the stored vial"
            );

            let fresh = store.add(draft("2026-08-13", 2500, Site::LeftThigh));
            assert_ne!(
                fresh,
                DoseId::new(3),
                "a fresh dose cannot collide with a stored one"
            );
        });
    }

    #[test]
    fn the_storage_scripts_agree_on_the_key() {
        const KEY: &str = "\"atlas.v1\"";
        assert!(READ.contains(KEY), "the read script names another key");
        assert!(WRITE.contains(KEY), "the write script names another key");
    }

    #[test]
    fn the_cycle_counts_down_to_the_next_dose_and_then_past_it() {
        let last = date("2026-08-06");
        let on = |today: &str| Cycle::new(last, date(today), 7).expect("a date on the calendar");

        assert_eq!(on("2026-08-06").remaining, 7);
        assert_eq!(on("2026-08-06").ring(), (7, "days"));
        assert_eq!(on("2026-08-06").fill(), 0);

        assert_eq!(on("2026-08-12").ring(), (1, "day"), "the day before");
        assert_eq!(on("2026-08-13").remaining, 0, "due seven days on");
        assert_eq!(on("2026-08-13").ring(), (0, "days"));
        assert_eq!(on("2026-08-13").fill(), 100);
        assert_eq!(on("2026-08-13").due, date("2026-08-13"));

        assert_eq!(on("2026-08-15").ring(), (2, "late"));
        assert_eq!(
            on("2026-08-15").fill(),
            100,
            "the ring stops when it is full"
        );
    }

    #[test]
    fn a_daily_drug_runs_a_one_day_cycle() {
        let cycle = Cycle::new(date("2026-08-06"), date("2026-08-06"), 1).expect("on the calendar");
        assert_eq!(cycle.due, date("2026-08-07"));
        assert_eq!(cycle.ring(), (1, "day"));
        assert_eq!(cycle.fill(), 0);
    }

    #[test]
    fn the_dosing_interval_follows_the_drug_and_falls_back_to_weekly() {
        assert_eq!(Medication::default().interval_days(), 7);
        for drug in Drug::ALL {
            let medication = Medication {
                drug: Some(drug),
                ..Medication::default()
            };
            assert_eq!(medication.interval_days(), drug.interval_days());
        }
    }

    #[test]
    fn a_plan_reports_the_week_and_step_a_date_falls_on() {
        let plan = [
            TitrationStep {
                micrograms: 250,
                weeks: 4,
            },
            TitrationStep {
                micrograms: 500,
                weeks: 4,
            },
        ];
        let started = date("2026-01-01");
        let at = |today: &str| rung(&plan, started, date(today));

        let first = at("2026-01-01").expect("the plan opens on its first day");
        assert_eq!((first.step, first.week, first.weeks), (1, 1, 4));
        assert_eq!(first.micrograms, 250);
        assert_eq!(first.steps, 2);

        let last_of_first = at("2026-01-28").expect("day 28 is still week 4");
        assert_eq!((last_of_first.step, last_of_first.week), (1, 4));

        let second = at("2026-01-29").expect("day 29 opens the second step");
        assert_eq!((second.step, second.week), (2, 1));
        assert_eq!(second.micrograms, 500);

        assert_eq!(at("2026-02-26"), None, "the plan runs out after 8 weeks");
    }

    #[test]
    fn a_date_before_a_plan_starts_reads_as_its_first_week() {
        let plan = [TitrationStep {
            micrograms: 250,
            weeks: 4,
        }];
        let position = rung(&plan, date("2026-01-08"), date("2026-01-01"));
        assert_eq!(position.map(|rung| (rung.step, rung.week)), Some((1, 1)));
    }

    #[test]
    fn an_empty_plan_has_nowhere_to_be() {
        assert_eq!(rung(&[], date("2026-01-01"), date("2026-02-01")), None);
    }

    #[test]
    fn a_dose_dated_ahead_leaves_the_ring_empty_rather_than_underfilled() {
        let cycle = Cycle::new(date("2026-08-20"), date("2026-08-06"), 7).expect("on the calendar");
        assert_eq!(cycle.remaining, 21);
        assert_eq!(cycle.fill(), 0);
    }

    #[test]
    fn the_suggested_site_rotates_away_from_the_ones_last_used() {
        store(|mut store| {
            assert_eq!(
                suggest_site(&store.all()),
                Site::ALL[0],
                "the first dose has nothing to rotate from"
            );

            for site in Site::ALL {
                let suggested = suggest_site(&store.all());
                assert_eq!(suggested, site, "logging in a row walks the sites in order");
                store.add(draft("2026-08-06", 2500, suggested));
            }

            assert_eq!(
                suggest_site(&store.all()),
                Site::ALL[0],
                "and comes back round to the one used longest ago"
            );
        });
    }

    #[test]
    fn a_site_never_used_is_suggested_ahead_of_one_used_long_ago() {
        store(|mut store| {
            store.add(draft("2026-07-02", 2500, Site::RightThigh));
            store.add(draft("2026-07-09", 2500, Site::LeftAbdomen));
            store.add(draft("2026-07-16", 2500, Site::RightAbdomen));

            assert_eq!(suggest_site(&store.all()), Site::LeftThigh);
        });
    }

    fn vials(micrograms: u32, count: u32) -> StockDraft {
        StockDraft {
            drug: Some(Drug::Tirzepatide),
            label: String::new(),
            micrograms,
            form: Form::Lyophilized,
            sealed: count,
            note: String::new(),
        }
    }

    fn drawn(taken: &str, micrograms: u32, vial: VialId) -> Draft {
        Draft {
            from: vec![Drawn { vial, micrograms }],
            ..draft(taken, micrograms, Site::LeftThigh)
        }
    }

    fn split(taken: &str, shares: &[(VialId, u32)]) -> Draft {
        let micrograms = shares.iter().map(|(_, share)| share).sum();
        Draft {
            from: shares
                .iter()
                .map(|&(vial, micrograms)| Drawn { vial, micrograms })
                .collect(),
            ..draft(taken, micrograms, Site::LeftThigh)
        }
    }

    fn only(store: Store) -> Stock {
        let shelf = store.stock();
        assert_eq!(shelf.len(), 1, "the shelf holds one entry");
        shelf.into_iter().next().expect("and it is that one")
    }

    #[test]
    fn ten_of_one_vial_are_one_entry_counting_ten() {
        store(|mut store| {
            let first = store.add_stock(vials(30_000, 4));
            let again = store.add_stock(vials(30_000, 6));

            assert_eq!(again, first, "the second lot lands on the first entry");
            assert_eq!(only(store).sealed, 10);
            assert_eq!(store.supply(), (10, 0));
        });
    }

    #[test]
    fn vials_that_are_not_the_same_thing_stand_as_their_own_entries() {
        store(|mut store| {
            store.add_stock(vials(30_000, 1));

            store.add_stock(vials(15_000, 1));
            store.add_stock(StockDraft {
                drug: Some(Drug::Semaglutide),
                ..vials(30_000, 1)
            });
            store.add_stock(StockDraft {
                label: "Batch B".to_owned(),
                ..vials(30_000, 1)
            });
            store.add_stock(StockDraft {
                form: Form::Solution { microlitres: 3000 },
                ..vials(30_000, 1)
            });

            assert_eq!(
                store.stock().len(),
                5,
                "a strength, a drug, a label and a form each tell two vials apart"
            );
        });
    }

    #[test]
    fn a_label_written_in_another_case_is_the_same_label() {
        store(|mut store| {
            store.add_stock(StockDraft {
                label: "Batch B".to_owned(),
                ..vials(30_000, 1)
            });
            store.add_stock(StockDraft {
                label: "  batch b ".to_owned(),
                ..vials(30_000, 1)
            });

            assert_eq!(only(store).sealed, 2);
        });
    }

    #[test]
    fn mixing_a_vial_takes_it_out_of_the_count_and_puts_a_full_one_in_use() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 10));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("a sealed vial is there to mix");

            let entry = only(store);
            assert_eq!(entry.sealed, 9, "the powder is one vial down");
            assert_eq!(entry.open.len(), 1);

            let mixed = &entry.open[0];
            assert_eq!(mixed.id, vial);
            assert_eq!(mixed.micrograms, 30_000, "it holds what the label said");
            assert_eq!(mixed.microlitres, 2000);
            assert_eq!(mixed.opened, date("2026-08-12"));
            assert_eq!(store.remaining(mixed), 30_000, "and nothing is out of it");
            assert_eq!(store.supply(), (9, 1));
        });
    }

    #[test]
    fn a_vial_cannot_be_mixed_out_of_an_empty_shelf() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 1));
            store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("the one");

            assert_eq!(store.open_vial(id, 2000, date("2026-08-19")), None);
            assert_eq!(only(store).open.len(), 1, "nothing was mixed twice");
        });
    }

    #[test]
    fn a_vial_falls_only_by_the_doses_drawn_from_it() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 2));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let other = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");

            store.add(drawn("2026-08-13", 2500, vial));
            store.add(draft("2026-08-20", 2500, Site::LeftThigh));

            let entry = only(store);
            assert_eq!(store.remaining(&entry.open[0]), 27_500);
            assert_eq!(
                store.remaining(&entry.open[1]),
                30_000,
                "a dose naming no vial leaves every vial alone"
            );
            assert_eq!(entry.open[1].id, other);
            assert_eq!(store.held(&entry), 27_500 + 30_000);
        });
    }

    #[test]
    fn a_dose_split_across_two_vials_falls_out_of_each_by_its_own_share() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 2));
            let first = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let second = store
                .open_vial(id, 2000, date("2026-08-19"))
                .expect("mixed");
            store.add(drawn("2026-08-13", 28_000, first));

            let dose = store.add(split("2026-08-20", &[(first, 2000), (second, 500)]));

            let entry = only(store);
            assert_eq!(store.remaining(&entry.open[0]), 0, "the first is emptied");
            assert_eq!(store.remaining(&entry.open[1]), 29_500);
            assert_eq!(
                store.get(dose).map(|dose| dose.micrograms),
                Some(2500),
                "and the dose is the whole of what was given"
            );
        });
    }

    #[test]
    fn discarding_one_vial_of_a_split_dose_leaves_the_other_share_standing() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 2));
            let first = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let second = store
                .open_vial(id, 2000, date("2026-08-19"))
                .expect("mixed");
            let dose = store.add(split("2026-08-20", &[(first, 2000), (second, 500)]));

            store.discard_vial(id, first);

            let logged = store.get(dose).expect("the dose is still logged");
            assert_eq!(logged.micrograms, 2500, "exactly as it was given");
            assert_eq!(
                logged.from,
                vec![Drawn {
                    vial: second,
                    micrograms: 500
                }],
                "naming only the vial still on the shelf"
            );
            assert_eq!(store.remaining(&only(store).open[0]), 29_500);
        });
    }

    #[test]
    fn a_split_dose_survives_being_written_and_read_back() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 2));
            let first = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let second = store
                .open_vial(id, 2000, date("2026-08-19"))
                .expect("mixed");
            store.add(split("2026-08-20", &[(first, 2000), (second, 500)]));
            let doses = store.all();

            let payload = serde_json::to_string(&Writing {
                version: VERSION,
                doses: doses.iter().map(Into::into).collect(),
                stock: store.stock().iter().map(Into::into).collect(),
                medication: &store.medication(),
                clock: store.clock(),
            })
            .expect("the document serialises");
            let read: Reading = serde_json::from_str(&payload).expect("and reads back");

            let logged: Vec<Dose> = read.doses.into_iter().map(Dose::from).collect();
            assert_eq!(logged, doses);
        });
    }

    #[test]
    fn deleting_a_dose_gives_the_drug_back_and_editing_one_reckons_it_afresh() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 1));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let dose = store.add(drawn("2026-08-13", 2500, vial));

            let held = |store: Store| store.remaining(&only(store).open[0]);
            assert_eq!(held(store), 27_500);

            store.update(dose, drawn("2026-08-13", 5000, vial));
            assert_eq!(held(store), 25_000, "the new strength is the one counted");

            store.update(dose, draft("2026-08-13", 5000, Site::LeftThigh));
            assert_eq!(held(store), 30_000, "unpicking the vial returns all of it");

            store.update(dose, drawn("2026-08-13", 5000, vial));
            store.remove(dose);
            assert_eq!(held(store), 30_000, "and so does deleting the dose");
        });
    }

    #[test]
    fn drawing_past_what_a_vial_holds_leaves_it_empty_rather_than_owing() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 1));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            store.add(drawn("2026-08-13", 25_000, vial));
            store.add(drawn("2026-08-20", 25_000, vial));

            let entry = only(store);
            assert_eq!(store.remaining(&entry.open[0]), 0);
            assert_eq!(store.held(&entry), 0);
        });
    }

    #[test]
    fn discarding_a_vial_leaves_the_doses_drawn_from_it_naming_nothing() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 1));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let dose = store.add(drawn("2026-08-13", 2500, vial));

            store.discard_vial(id, vial);

            assert!(only(store).open.is_empty());
            let logged = store.get(dose).expect("the dose is still logged");
            assert_eq!(logged.micrograms, 2500, "exactly as it was given");
            assert!(logged.from.is_empty(), "and pointing at no vial");
            assert_eq!(store.vial(vial), None);
        });
    }

    #[test]
    fn discarding_against_the_wrong_entry_leaves_both_of_them_alone() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 1));
            let other = store.add_stock(vials(15_000, 1));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let dose = store.add(drawn("2026-08-13", 2500, vial));

            store.discard_vial(other, vial);

            let entry = store.stock_of(id).expect("the entry still holds it");
            assert_eq!(entry.open.len(), 1, "the vial is on the shelf it was on");
            assert_eq!(
                store.remaining(&entry.open[0]),
                27_500,
                "and still counts the dose drawn from it"
            );
            assert_eq!(
                store.get(dose).map(|dose| dose.from),
                Some(vec![Drawn {
                    vial,
                    micrograms: 2500
                }])
            );
        });
    }

    #[test]
    fn deleting_an_entry_takes_the_vials_in_use_with_it() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 2));
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let dose = store.add(drawn("2026-08-13", 2500, vial));

            store.remove_stock(id);

            assert!(store.stock().is_empty());
            assert_eq!(store.supply(), (0, 0));
            assert_eq!(store.get(dose).map(|dose| dose.from), Some(Vec::new()));
        });
    }

    #[test]
    fn editing_an_entry_leaves_the_vials_already_in_use_holding_what_they_did() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 3));
            store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");

            store.update_stock(id, vials(15_000, 1));

            let entry = only(store);
            assert_eq!(entry.micrograms, 15_000);
            assert_eq!(entry.sealed, 1);
            assert_eq!(
                entry.open[0].micrograms, 30_000,
                "a vial already mixed was made up at the old strength"
            );
        });
    }

    #[test]
    fn a_stock_id_is_never_handed_out_twice() {
        store(|mut store| {
            let first = store.add_stock(vials(30_000, 1));
            store.remove_stock(first);
            let second = store.add_stock(vials(30_000, 1));

            assert_ne!(first, second, "a deleted entry does not lend out its id");
            assert_eq!(store.stock_of(first), None);
        });
    }

    #[test]
    fn the_shelf_and_the_vial_a_dose_came_from_survive_being_written_and_read_back() {
        store(|mut store| {
            let id = store.add_stock(StockDraft {
                note: "Batch 42".to_owned(),
                ..vials(30_000, 9)
            });
            let vial = store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            store.add(drawn("2026-08-13", 2500, vial));
            let doses = store.all();
            let shelf = store.stock();

            let payload = serde_json::to_string(&Writing {
                version: VERSION,
                doses: doses.iter().map(Into::into).collect(),
                stock: shelf.iter().map(Into::into).collect(),
                medication: &store.medication(),
                clock: store.clock(),
            })
            .expect("the document serialises");
            let read: Reading = serde_json::from_str(&payload).expect("and reads back");

            let back: Vec<Stock> = read.stock.into_iter().map(Stock::from).collect();
            let logged: Vec<Dose> = read.doses.into_iter().map(Dose::from).collect();
            assert_eq!(back, shelf);
            assert_eq!(logged, doses);
            assert_eq!(
                logged[0].from,
                vec![Drawn {
                    vial,
                    micrograms: 2500
                }]
            );
        });
    }

    #[test]
    fn a_shelf_read_back_hands_out_ids_past_everything_on_it() {
        store(|mut store| {
            let id = store.add_stock(vials(30_000, 2));
            store
                .open_vial(id, 2000, date("2026-08-12"))
                .expect("mixed");
            let shelf = store.stock();

            let mut restarted = Store::new();
            restarted.install(Vec::new(), shelf);

            let fresh = restarted.add_stock(vials(15_000, 1));
            assert_ne!(fresh, id, "a fresh entry cannot collide with a stored one");

            let mixed = restarted
                .open_vial(id, 2000, date("2026-08-19"))
                .expect("the second vial is still sealed");
            let entry = restarted.stock_of(id).expect("the stored entry");
            assert_ne!(
                entry.open[0].id, mixed,
                "nor a fresh vial with one read back"
            );
        });
    }
}
