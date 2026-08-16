//! Everything Atlas keeps: the dose log, the medication being tracked, and the device storage
//! behind both.
//!
//! The two are held in signals and written back to the device as one document after every
//! change, by [`use_store`]. Nothing leaves the device.

use std::cmp::{Ordering, Reverse};

use chrono::{Days, Local, NaiveDate, NaiveTime, Weekday};
use dioxus::document;
use dioxus::prelude::*;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::formulary::Drug;

/// Days between doses when no drug has been chosen.
const DEFAULT_INTERVAL_DAYS: u32 = 7;

/// The largest strength that can be logged, in micrograms. Far past any GLP-1 dose; it is here
/// so a mistyped figure cannot become a stored one.
const MAX_MICROGRAMS: u32 = 1_000_000;

/// Bumped when the stored shape changes. A stored document carrying any other version is left
/// untouched rather than read, so a newer build's data survives an older one opening it.
const VERSION: u32 = 1;

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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DoseId(u64);

impl DoseId {
    /// Only the log mints these, so an id always names a dose that was really logged. The tests
    /// need one without a log to hand.
    #[cfg(test)]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Dose {
    pub id: DoseId,
    pub taken: NaiveDate,
    /// The time of day it was given, where that is known. Left optional because a dose entered
    /// from memory has a date and no hour, and inventing one would put a figure in the record the
    /// user never gave. The level model places a dose without one at [`ASSUMED_HOUR`].
    #[serde(default)]
    pub time: Option<NaiveTime>,
    /// Which drug this injection was, per dose rather than per person: a log spans a switch from
    /// one drug to another, and each clears at its own rate.
    ///
    /// Unset where the drug was never chosen. Such a dose stays in the log and out of every
    /// curve, since nothing can be modelled without knowing the molecule.
    #[serde(default)]
    pub drug: Option<Drug>,
    /// Strength in micrograms. Whole numbers, because doses are compared and differenced, and a
    /// tenth of a milligram is a real difference that a binary fraction does not hold exactly.
    pub micrograms: u32,
    pub site: Site,
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
    pub note: String,
}

pub fn assumed_time() -> NaiveTime {
    NaiveTime::from_hms_opt(ASSUMED_HOUR, 0, 0).expect("midday is a time of day")
}

/// The time of day, where the device is.
pub fn time_of_day() -> NaiveTime {
    Local::now().time()
}

/// Which face the clock shows: whether a time of day reads `14:00` or `2:00 PM`. Chosen by the
/// user; the platform's locale is never asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Clock {
    #[default]
    TwentyFourHour,
    TwelveHour,
}

impl Clock {
    pub const ALL: [Clock; 2] = [Clock::TwentyFourHour, Clock::TwelveHour];

    pub fn label(self) -> &'static str {
        match self {
            Clock::TwentyFourHour => "24-hour",
            Clock::TwelveHour => "AM / PM",
        }
    }

    /// How `chrono` writes a time of day on this face.
    pub fn face(self) -> &'static str {
        match self {
            Clock::TwentyFourHour => "%H:%M",
            // `%-I` rather than `%I`, so one in the afternoon is `1:00 PM` and not `01:00 PM`.
            Clock::TwelveHour => "%-I:%M %p",
        }
    }
}

/// Reads a time as a time input reports it. An empty box is a dose whose hour was not recorded,
/// and reads as `None` along with anything unreadable.
pub fn parse_time(text: &str) -> Option<NaiveTime> {
    let text = text.trim();
    NaiveTime::parse_from_str(text, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M:%S"))
        .ok()
}

/// A time of day, on the face the user chose.
pub fn format_time(time: NaiveTime, clock: Clock) -> String {
    time.format(clock.face()).to_string()
}

/// A time of day as an `<input type="time">` reports and expects it, which is 24-hour whatever
/// face the control draws itself with.
pub fn time_value(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
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

/// The stored document on the way out.
#[derive(Serialize)]
struct Writing<'a> {
    version: u32,
    doses: &'a [Dose],
    medication: &'a Medication,
    clock: Clock,
}

/// The stored document on the way in. Records added within a version default rather than fail,
/// so a document written before them still reads.
#[derive(Deserialize)]
struct Reading {
    version: u32,
    #[serde(default)]
    doses: Vec<Dose>,
    #[serde(default)]
    medication: Medication,
    #[serde(default)]
    clock: Clock,
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

/// Reads a strength in milligrams as typed, in micrograms. Rejects anything that is not a
/// positive figure with at most three decimals. A trailing `mg` is accepted and ignored.
pub fn parse_mg(text: &str) -> Option<u32> {
    let text = text.trim().trim_end_matches("mg").trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if text.is_empty() || fraction.len() > 3 || !digits(whole) || !digits(fraction) {
        return None;
    }

    let whole: u32 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: u32 = format!("{fraction:0<3}").parse().ok()?;
    let micrograms = whole.checked_mul(1000)?.checked_add(fraction)?;
    (1..=MAX_MICROGRAMS)
        .contains(&micrograms)
        .then_some(micrograms)
}

/// Milligrams, written back the way they were entered: no trailing zeros, no point when whole.
pub fn format_mg(micrograms: u32) -> String {
    let whole = micrograms / 1000;
    let fraction = micrograms % 1000;
    if fraction == 0 {
        format!("{whole}")
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_owned()
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
    medication: Signal<Medication>,
    /// The face every time of day in the app is written on.
    clock: Signal<Clock>,
    status: Signal<Status>,
    /// Never reused within a session, so an id held by an open page cannot come to name a dose
    /// logged after it. Not stored: nothing that holds one outlives a restart.
    next_id: Signal<u64>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            doses: Signal::new(Vec::new()),
            medication: Signal::new(Medication::default()),
            clock: Signal::new(Clock::default()),
            status: Signal::new(Status::Loading),
            next_id: Signal::new(0),
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
        dose.note = draft.note;
        doses.sort_by(newest_first);
    }

    pub fn remove(&mut self, id: DoseId) {
        self.doses.write().retain(|dose| dose.id != id);
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

    /// Once, at startup.
    async fn hydrate(mut self) {
        let mut eval = document::eval(READ);
        let status = match eval.recv::<Fetched>().await {
            Ok(Fetched {
                ok: true,
                raw: None,
            }) => Status::Ready,
            Ok(Fetched {
                ok: true,
                raw: Some(raw),
            }) => match serde_json::from_str::<Reading>(&raw) {
                Ok(reading) if reading.version == VERSION => {
                    self.install(reading.doses);
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

    /// Puts a log read from the device in place, leaving the id counter past everything in it.
    fn install(&mut self, mut doses: Vec<Dose>) {
        doses.sort_by(newest_first);
        let next = doses
            .iter()
            .map(|dose| dose.id.0)
            .max()
            .map_or(0, |highest| highest.saturating_add(1));
        self.next_id.set(next);
        self.doses.set(doses);
    }

    /// Silent unless the stored log has been read: an empty starting log must not land on top of
    /// one, and a log this build cannot parse must not be replaced by one it can.
    fn persist(self, writer: Coroutine<String>) {
        if self.status() != Status::Ready {
            return;
        }
        let doses = self.doses.read();
        let medication = self.medication.read();
        let Ok(payload) = serde_json::to_string(&Writing {
            version: VERSION,
            doses: &doses,
            medication: &medication,
            clock: *self.clock.read(),
        }) else {
            return;
        };
        writer.send(payload);
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
                note: String::new(),
            });
            let evening = store.add(Draft {
                taken: date("2026-08-06"),
                time: Some(time("21:00")),
                drug: None,
                micrograms: 2500,
                site: Site::RightThigh,
                note: String::new(),
            });

            let order: Vec<_> = store.all().iter().map(|dose| dose.id).collect();
            assert_eq!(order, vec![evening, morning], "the later hour leads");
        });
    }

    #[test]
    fn a_stored_dose_without_an_hour_reads_back_as_one_that_never_had_it() {
        let payload = r#"{"version":1,"doses":[
            {"id":0,"taken":"2026-08-06","micrograms":2500,"site":"LeftThigh","note":""}
        ]}"#;
        let read: Reading = serde_json::from_str(payload).expect("it still reads");

        assert_eq!(read.doses.len(), 1);
        assert_eq!(read.doses[0].time, None);
        assert_eq!(read.doses[0].hour(), assumed_time());
    }

    #[test]
    fn an_hour_survives_being_written_and_read_back_on_either_face() {
        for (text, twelve) in [
            ("00:00", "12:00 AM"),
            ("07:30", "7:30 AM"),
            ("12:00", "12:00 PM"),
            ("13:05", "1:05 PM"),
            ("23:59", "11:59 PM"),
        ] {
            let parsed = parse_time(text).expect("a time a clock shows");

            assert_eq!(format_time(parsed, Clock::TwentyFourHour), text);
            assert_eq!(format_time(parsed, Clock::TwelveHour), twelve);
            // The form's own box speaks 24-hour whatever face the rest of the app wears.
            assert_eq!(time_value(parsed), text);
        }
    }

    #[test]
    fn an_empty_or_unreadable_time_is_a_dose_with_no_hour() {
        for text in ["", "   ", "half past seven", "25:00", "7"] {
            assert_eq!(parse_time(text), None, "`{text}` is not a time");
        }
    }

    #[test]
    fn a_time_input_reporting_seconds_is_still_read() {
        assert_eq!(parse_time("07:30:00"), Some(time("07:30")));
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
            let doses = store.all();
            let medication = store.medication();

            let payload = serde_json::to_string(&Writing {
                version: VERSION,
                doses: &doses,
                medication: &medication,
                clock: store.clock(),
            })
            .expect("the document serialises");
            let read: Reading = serde_json::from_str(&payload).expect("and reads back");

            assert_eq!(read.version, VERSION);
            assert_eq!(read.doses, doses);
            assert_eq!(read.medication, medication);
            assert_eq!(read.clock, Clock::TwelveHour);
        });
    }

    #[test]
    fn a_document_missing_a_record_reads_with_that_record_empty() {
        let payload = r#"{"version":1,"doses":[]}"#;
        let read: Reading = serde_json::from_str(payload).expect("it still reads");

        assert_eq!(read.medication, Medication::default());
        assert!(read.doses.is_empty());
        assert_eq!(
            read.clock,
            Clock::TwentyFourHour,
            "a document written before the setting reads as the app behaved then"
        );
    }

    #[test]
    fn a_stored_log_from_another_version_is_not_read() {
        let payload = r#"{"version":99,"doses":[]}"#;
        let read: Reading = serde_json::from_str(payload).expect("it is still json");
        assert_ne!(read.version, VERSION);
    }

    #[test]
    fn the_storage_scripts_agree_on_the_key() {
        const KEY: &str = "\"atlas.v1\"";
        assert!(READ.contains(KEY), "the read script names another key");
        assert!(WRITE.contains(KEY), "the write script names another key");
    }

    #[test]
    fn strengths_survive_being_written_and_read_back() {
        for (text, micrograms) in [
            ("2.5", 2500),
            ("0.25", 250),
            ("1", 1000),
            ("15", 15_000),
            ("1.7", 1700),
            ("0.025", 25),
        ] {
            assert_eq!(parse_mg(text), Some(micrograms), "reading {text}");
            assert_eq!(format_mg(micrograms), text, "writing {text} back");
        }
    }

    #[test]
    fn strengths_are_read_however_they_are_typed() {
        assert_eq!(parse_mg(" 2.5 mg "), Some(2500));
        assert_eq!(parse_mg("2.50"), Some(2500), "trailing zeros are harmless");
        assert_eq!(parse_mg(".5"), Some(500));
    }

    #[test]
    fn a_strength_that_is_not_a_dose_is_refused() {
        for text in [
            "", " ", "mg", "-1", "0", "0.0", "2.5.1", "2,5", "two", "2.0005", "1001", "1e3", "+2",
        ] {
            assert_eq!(parse_mg(text), None, "`{text}` is not a strength");
        }
    }

    #[test]
    fn the_largest_strength_is_accepted_and_anything_past_it_is_not() {
        assert_eq!(parse_mg("1000"), Some(MAX_MICROGRAMS));
        assert_eq!(parse_mg("1000.001"), None);
        assert_eq!(parse_mg("4294968"), None, "past what micrograms can hold");
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
}
