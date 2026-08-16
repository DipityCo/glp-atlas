//! Medication levels across the dosing cycle: how much of the drug is still in the body, and
//! where that settles once the doses stack up.
//!
//! A one-compartment model with first-order absorption and first-order elimination, superposed
//! over every dose. It reports micrograms that have reached the circulation, which needs only the
//! two rate constants; a blood concentration would need a volume of distribution and a
//! bioavailability besides, neither of which Atlas can know. Drug still in the tissue at the
//! injection site is not counted, so the figure climbs out of an injection rather than stepping
//! up at it.
//!
//! The axis runs in days from an origin date, in the device's own local time throughout. Nothing
//! carries a zone with it, so a log kept across a move between zones has the gap between two
//! doses out by the difference.
//!
//! The constants come from [`crate::formulary::Drug`] and are unverified there. Nothing below can
//! tell a wrong one from a right one: these tests check the arithmetic and take them as given.

use std::f64::consts::LN_2;

use chrono::{Local, NaiveDate, NaiveTime, Timelike};

use crate::formulary::Drug;
use crate::store::Dose;

/// A day count on the model's axis, saturating at `i32::MAX` rather than wrapping.
fn days(count: i64) -> f64 {
    f64::from(i32::try_from(count).unwrap_or(i32::MAX))
}

/// Whole days from `origin` to `date`.
pub fn day_of(date: NaiveDate, origin: NaiveDate) -> f64 {
    days((date - origin).num_days())
}

pub fn through_day(time: NaiveTime) -> f64 {
    f64::from(time.num_seconds_from_midnight()) / 86_400.0
}

/// Where a dose sits on the axis: the day it was given, plus how far into that day its hour
/// falls. A dose recorded without one is placed at [`crate::store::ASSUMED_HOUR`].
pub fn instant_of(dose: &Dose, origin: NaiveDate) -> f64 {
    day_of(dose.taken, origin) + through_day(dose.hour())
}

/// This moment on the same axis, carrying the time of day rather than stepping once a night.
pub fn moment(origin: NaiveDate) -> f64 {
    let now = Local::now();
    day_of(now.date_naive(), origin) + through_day(now.time())
}

/// The absorption rate, per day, that puts a single dose's peak at `time_to_peak`.
///
/// Solved rather than stored, since labels report a time to peak and not a `ka`. The peak is at
/// `t = ln(ka/ke) / (ka - ke)`, which has no closed form in `ka` and falls as `ka` rises, so it
/// is found by halving the interval.
///
/// The bracket assumes `ka > ke` and opens just above `ke`, where the expression is singular and
/// the peak runs off to `1 / ke`. A time to peak at or past that has no rate that produces it.
fn absorption_rate(elimination: f64, time_to_peak: f64) -> f64 {
    let peak_at = |absorption: f64| (absorption / elimination).ln() / (absorption - elimination);
    let (mut low, mut high) = (elimination * 1.000_001, elimination * 1e6);
    for _ in 0..100 {
        let middle = f64::midpoint(low, high);
        if peak_at(middle) > time_to_peak {
            low = middle;
        } else {
            high = middle;
        }
    }
    f64::midpoint(low, high)
}

/// The rate constants of the model, per day.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Kinetics {
    pub absorption: f64,
    pub elimination: f64,
}

impl Kinetics {
    pub fn of(drug: Drug) -> Self {
        let elimination = LN_2 / drug.elimination_half_life_days();
        Self {
            absorption: absorption_rate(elimination, drug.time_to_peak_days()),
            elimination,
        }
    }

    /// When a single dose reaches its peak, in days after it was given.
    pub fn time_to_peak(self) -> f64 {
        (self.absorption / self.elimination).ln() / (self.absorption - self.elimination)
    }

    /// Drug from one dose that has reached the circulation and not yet been cleared, `elapsed`
    /// days after the injection.
    ///
    /// Zero at the injection, where all of it is still in the tissue, and climbing from there as
    /// the depot empties. Peaks at [`Kinetics::time_to_peak`] and falls away behind it.
    pub fn after_dose(self, micrograms: u32, elapsed: f64) -> f64 {
        if elapsed <= 0.0 {
            return 0.0;
        }
        f64::from(micrograms)
            * self.scale()
            * ((-self.elimination * elapsed).exp() - (-self.absorption * elapsed).exp())
    }

    /// What the same dose, repeated every `interval` days for long enough, settles to `phase`
    /// days into a cycle: the limit where each cycle eliminates as much as the dose adds.
    pub fn at_steady_state(self, micrograms: u32, interval: f64, phase: f64) -> f64 {
        let accumulated = |rate: f64| (-rate * phase).exp() / (1.0 - (-rate * interval).exp());
        f64::from(micrograms)
            * self.scale()
            * (accumulated(self.elimination) - accumulated(self.absorption))
    }

    /// The highest a settled cycle reaches.
    ///
    /// Searched across the cycle rather than read at [`Kinetics::time_to_peak`]: the doses before
    /// this one are still clearing while it climbs, so their fall pulls the summed peak in front
    /// of where a lone dose would peak.
    pub fn peak_at_steady_state(self, micrograms: u32, interval: f64) -> f64 {
        let steps = 64;
        (0..=steps)
            .map(|step| {
                let phase = interval * f64::from(step) / f64::from(steps);
                self.at_steady_state(micrograms, interval, phase)
            })
            .fold(0.0_f64, f64::max)
    }

    fn scale(self) -> f64 {
        self.absorption / (self.absorption - self.elimination)
    }
}

/// The model works in these rather than in [`Dose`] so that a projected dose can go through it
/// alongside the ones that were really given.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Given {
    pub day: f64,
    pub micrograms: u32,
}

impl Given {
    /// The doses of one drug from the log, on the axis running from `origin`, each at the hour it
    /// was given.
    ///
    /// One drug at a time: two are two molecules with two clearances, and the milligrams of one
    /// cannot be added to the milligrams of the other. A dose with no drug belongs to no curve.
    pub fn from_log(doses: &[Dose], origin: NaiveDate, drug: Drug) -> Vec<Self> {
        doses
            .iter()
            .filter(|dose| dose.drug == Some(drug))
            .map(|dose| Self {
                day: instant_of(dose, origin),
                micrograms: dose.micrograms,
            })
            .collect()
    }
}

/// The level a curve is followed down to, in micrograms: a hundredth of a milligram, the finest
/// a level is ever printed at.
///
/// A bound on the timeline and not on the figures, and a drawing threshold rather than a clinical
/// one: the model goes on below it, and this is only where the chart stops following. The tail is
/// an asymptote that never arrives, so [`spent`] answers against this.
pub const TRACE: f64 = 10.0;

/// The first day from `after` at which the level is under [`TRACE`], or `cap` where it never is.
///
/// `after` must be the greatest day in `given`, which is not its last element: the log arrives
/// newest first and projected doses are appended after it. The walk starts a time-to-peak past
/// `after`, since at the injection none of it has reached the circulation and starting on the
/// dose reads every curve as spent the moment it was given. Walked a day at a time because a sum
/// of decaying exponentials has no closed form for where it crosses a threshold.
pub fn spent(kinetics: Kinetics, given: &[Given], after: f64, cap: f64) -> f64 {
    let mut day = after + kinetics.time_to_peak();
    // A walk from an infinity never advances.
    if !day.is_finite() {
        return cap;
    }
    while day < cap {
        if on_board(kinetics, given, day) < TRACE {
            return day;
        }
        day += 1.0;
    }
    cap
}

/// Drug in the circulation at a point on the axis, from every dose given before it.
///
/// Not the same as the drug in the body: a dose injected an hour ago is almost entirely still in
/// the tissue, counted here as nearly nothing because it has not reached the bloodstream yet.
pub fn on_board(kinetics: Kinetics, given: &[Given], at: f64) -> f64 {
    given
        .iter()
        .map(|dose| kinetics.after_dose(dose.micrograms, at - dose.day))
        .sum()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sample {
    pub day: f64,
    pub micrograms: f64,
}

/// Fractions of the time to peak that extra points are placed at after a dose, covering the rise
/// and the turn over the top of it.
const RISE: [f64; 11] = [0.0, 0.1, 0.2, 0.35, 0.5, 0.7, 0.85, 1.0, 1.3, 1.8, 2.5];

/// Past this many doses in one window the peaks are packed too close to read apart, and following
/// each one costs more than it shows.
const CROWDED: usize = 120;

/// `count` evenly spaced points across `from..=to`, and extra points around each dose.
///
/// Absorption is much the faster of the two rates, so even spacing wide enough to cover a year
/// steps straight over the rise, drawing a vertical jump at the injection and clipping the peak
/// behind it. The extra points follow each dose through its own rise.
pub fn curve(kinetics: Kinetics, given: &[Given], from: f64, to: f64, count: u32) -> Vec<Sample> {
    let count = count.max(2);
    let last = f64::from(count - 1);
    let mut days: Vec<f64> = (0..count)
        .map(|index| from + (to - from) * f64::from(index) / last)
        .collect();

    let peak = kinetics.time_to_peak();
    let following: Vec<&Given> = given
        .iter()
        .filter(|dose| dose.day > from - peak * RISE[RISE.len() - 1] && dose.day < to)
        .collect();
    if (to - from) / last > peak / 4.0 && following.len() <= CROWDED {
        for dose in following {
            days.extend(
                RISE.iter()
                    .map(|fraction| dose.day + peak * fraction)
                    .filter(|day| *day > from && *day < to),
            );
        }
        days.sort_by(f64::total_cmp);
    }

    days.into_iter()
        .map(|day| Sample {
            day,
            micrograms: on_board(kinetics, given, day),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{DoseId, Draft, Site, Store};
    use dioxus::prelude::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date the test wrote")
    }

    fn log(drug: Drug, strengths: &[(&str, u32)], f: impl FnOnce(Vec<Dose>)) {
        let dom = VirtualDom::new(|| rsx! { div {} });
        dom.in_runtime(|| {
            dioxus::core::Runtime::current().in_scope(ScopeId::ROOT, || {
                let mut store = Store::new();
                for (taken, micrograms) in strengths {
                    store.add(Draft {
                        taken: date(taken),
                        time: None,
                        drug: Some(drug),
                        micrograms: *micrograms,
                        site: Site::LeftAbdomen,
                        note: String::new(),
                    });
                }
                f(store.all());
            });
        });
    }

    #[test]
    fn every_drug_peaks_at_the_time_its_own_constant_declares() {
        for drug in Drug::ALL {
            let kinetics = Kinetics::of(drug);
            let target = drug.time_to_peak_days();
            let peak = kinetics.time_to_peak();

            assert!(
                (peak - target).abs() < 1e-6,
                "{} peaks at {peak} days, not the {target} it declares",
                drug.label()
            );
            assert!(
                kinetics.absorption > kinetics.elimination * 1.05,
                "{} absorbs no faster than it clears",
                drug.label()
            );
        }
    }

    // The zeros are the literal the function returns, not a figure it arrived at.
    #[test]
    #[allow(clippy::float_cmp)]
    fn a_dose_is_absent_before_it_is_given_and_peaks_once() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let peak = Drug::Semaglutide.time_to_peak_days();

        assert_eq!(kinetics.after_dose(2500, -1.0), 0.0);
        assert_eq!(kinetics.after_dose(2500, 0.0), 0.0);

        let at_peak = kinetics.after_dose(2500, peak);
        assert!(at_peak > kinetics.after_dose(2500, peak - 0.5));
        assert!(at_peak > kinetics.after_dose(2500, peak + 0.5));
        assert!(
            at_peak < 2500.0,
            "no more can be on board than was injected"
        );
    }

    #[test]
    fn what_is_left_halves_over_a_half_life_once_absorption_is_done() {
        let drug = Drug::Semaglutide;
        let kinetics = Kinetics::of(drug);
        let half_life = drug.elimination_half_life_days();
        // Far enough past the peak that the absorption term has run out.
        let start = kinetics.after_dose(2500, 20.0);
        let later = kinetics.after_dose(2500, 20.0 + half_life);

        assert!((later / start - 0.5).abs() < 0.001, "{start} to {later}");
    }

    #[test]
    fn doses_add_up() {
        let kinetics = Kinetics::of(Drug::Tirzepatide);
        log(
            Drug::Tirzepatide,
            &[("2026-08-01", 5000), ("2026-08-08", 5000)],
            |doses| {
                let given = Given::from_log(&doses, date("2026-08-01"), Drug::Tirzepatide);
                let total = on_board(kinetics, &given, 10.0);
                // Neither dose recorded an hour, so both sit half a day into their own.
                let separately = kinetics.after_dose(5000, 9.5) + kinetics.after_dose(5000, 2.5);

                assert!((total - separately).abs() < 1e-9);
            },
        );
    }

    #[test]
    fn a_dose_nobody_follows_is_spent_well_inside_the_cap() {
        for drug in Drug::ALL {
            let kinetics = Kinetics::of(drug);
            let given = [Given {
                day: 0.0,
                micrograms: 2500,
            }];
            let cap = 84.0;
            let ends = spent(kinetics, &given, 0.0, cap);

            assert!(ends < cap, "{drug:?} still had something left at the cap");
            assert!(
                on_board(kinetics, &given, ends) < TRACE,
                "{drug:?} was called spent with something still in it"
            );
            assert!(
                on_board(kinetics, &given, ends - 1.0) >= TRACE,
                "{drug:?} was called spent a day before it ran out"
            );
        }
    }

    #[test]
    fn a_drug_with_no_doses_is_spent_where_it_is_asked() {
        let kinetics = Kinetics::of(Drug::Semaglutide);

        assert!(spent(kinetics, &[], 0.0, 84.0) < 84.0, "nothing to run out");
        assert!(
            (spent(kinetics, &[], f64::NEG_INFINITY, 84.0) - 84.0).abs() < 1e-9,
            "and a walk from no dose at all ends rather than running forever"
        );
    }

    #[test]
    fn a_dose_repeated_on_schedule_is_never_spent() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let given: Vec<Given> = (0..12)
            .map(|cycle| Given {
                day: f64::from(cycle) * 7.0,
                micrograms: 2500,
            })
            .collect();

        assert!((spent(kinetics, &given, 77.0, 84.0) - 84.0).abs() < 1e-9);
    }

    #[test]
    fn a_curve_is_drawn_at_what_the_model_says_all_the_way_down() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let given = [Given {
            day: 0.0,
            micrograms: 2500,
        }];
        let ends = spent(kinetics, &given, 0.0, 84.0);
        let samples = curve(kinetics, &given, ends - 4.0, ends, 5);

        for pair in samples.windows(2) {
            let (above, below) = (pair[0].micrograms, pair[1].micrograms);
            assert!(below < above, "the tail keeps falling");
            assert!(below > 0.0, "and never lands on nothing");
            assert!(
                above - below < TRACE,
                "no step the size of the trace: {above} to {below}"
            );
        }
    }

    #[test]
    fn a_curve_takes_only_the_doses_of_its_own_drug() {
        let dom = VirtualDom::new(|| rsx! { div {} });
        dom.in_runtime(|| {
            dioxus::core::Runtime::current().in_scope(ScopeId::ROOT, || {
                let mut store = Store::new();
                let mut draft = |drug: Option<Drug>, taken: &str| {
                    store.add(Draft {
                        taken: date(taken),
                        time: None,
                        drug,
                        micrograms: 2500,
                        site: Site::LeftAbdomen,
                        note: String::new(),
                    });
                };
                draft(Some(Drug::Semaglutide), "2026-08-01");
                draft(Some(Drug::Tirzepatide), "2026-08-08");
                draft(None, "2026-08-15");

                let doses = store.all();
                let origin = date("2026-08-01");

                assert_eq!(
                    Given::from_log(&doses, origin, Drug::Semaglutide).len(),
                    1,
                    "a curve takes its own doses"
                );
                assert_eq!(Given::from_log(&doses, origin, Drug::Tirzepatide).len(), 1);
                assert_eq!(
                    Given::from_log(&doses, origin, Drug::Retatrutide).len(),
                    0,
                    "and a drug with no doses has no curve to draw"
                );
                assert_eq!(doses.len(), 3, "the dose with no drug is still in the log");
            });
        });
    }

    #[test]
    fn a_dose_still_to_come_goes_through_the_model_like_any_other() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let real = Given {
            day: 0.0,
            micrograms: 2500,
        };
        let scheduled = Given {
            day: 7.0,
            micrograms: 5000,
        };

        let at_ten = on_board(kinetics, &[real, scheduled], 10.0);
        assert!(
            at_ten > on_board(kinetics, &[real], 10.0),
            "the dose due on day seven is on board by day ten"
        );
        assert!(
            (on_board(kinetics, &[real, scheduled], 5.0) - on_board(kinetics, &[real], 5.0)).abs()
                < 1e-12,
            "and counts for nothing before it"
        );
    }

    #[test]
    fn the_steady_state_is_where_repeated_dosing_arrives() {
        for drug in Drug::ALL {
            let kinetics = Kinetics::of(drug);
            let interval = f64::from(drug.interval_days());
            let phase = interval * 0.4;
            // Two hundred cycles is far past any half-life here.
            let simulated: f64 = (0..200)
                .map(|cycle| kinetics.after_dose(2500, phase + interval * f64::from(cycle)))
                .sum();
            let settled = kinetics.at_steady_state(2500, interval, phase);

            assert!(
                (simulated - settled).abs() < 1e-6,
                "{}: {simulated} against {settled}",
                drug.label()
            );
        }
    }

    #[test]
    fn a_settled_cycle_peaks_in_front_of_where_a_lone_dose_would() {
        for drug in Drug::ALL {
            let kinetics = Kinetics::of(drug);
            let interval = f64::from(drug.interval_days());
            let searched = kinetics.peak_at_steady_state(2500, interval);
            let at_lone_peak =
                kinetics.at_steady_state(2500, interval, drug.time_to_peak_days().min(interval));

            assert!(
                searched >= at_lone_peak,
                "{}: searched {searched} is under {at_lone_peak}",
                drug.label()
            );
            for step in 0..=32 {
                let phase = interval * f64::from(step) / 32.0;
                assert!(kinetics.at_steady_state(2500, interval, phase) <= searched * 1.0001);
            }
        }
    }

    #[test]
    fn a_level_climbs_towards_steady_state_and_does_not_pass_it() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let settled = kinetics.at_steady_state(2500, 7.0, 7.0);
        let mut previous = 0.0;

        for cycles in 1..30 {
            let trough: f64 = (1..=cycles)
                .map(|earlier| kinetics.after_dose(2500, f64::from(earlier) * 7.0))
                .sum();
            assert!(trough > previous, "the trough climbs cycle on cycle");
            assert!(trough <= settled, "and never passes where it settles");
            previous = trough;
        }
        assert!(
            previous > settled * 0.99,
            "and is all but there after thirty cycles"
        );
    }

    #[test]
    fn a_weekly_drug_with_a_weekly_half_life_settles_at_about_twice_one_dose() {
        let drug = Drug::Semaglutide;
        let kinetics = Kinetics::of(drug);
        let peak = drug.time_to_peak_days();

        let one = kinetics.after_dose(2500, peak);
        let settled = kinetics.at_steady_state(2500, 7.0, peak);

        assert!(
            (1.9..2.2).contains(&(settled / one)),
            "accumulates {}x over one dose",
            settled / one
        );
        assert!(
            (4000.0..4500.0).contains(&settled),
            "{settled} micrograms on board at the peak of 2.5 mg weekly"
        );
        assert!(
            kinetics.at_steady_state(2500, 7.0, 7.0) < settled,
            "the trough sits under the peak"
        );
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn the_curve_spans_the_window_it_was_asked_for() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        log(Drug::Semaglutide, &[("2026-08-06", 2500)], |doses| {
            let given = Given::from_log(&doses, date("2026-08-06"), Drug::Semaglutide);
            let samples = curve(kinetics, &given, -1.0, 7.0, 9);
            let last = samples.last().expect("a curve has points");

            assert!(samples.len() >= 9, "at least the points it was asked for");
            assert!((samples[0].day - -1.0).abs() < 1e-9);
            assert!((last.day - 7.0).abs() < 1e-9);
            assert_eq!(
                samples[0].micrograms, 0.0,
                "nothing has reached the circulation before the first dose"
            );
            assert!(last.micrograms > 0.0);
        });
    }

    #[test]
    fn a_curve_runs_in_order_whatever_extra_points_it_takes() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let given: Vec<Given> = (0..8)
            .map(|week| Given {
                day: f64::from(week) * 7.0,
                micrograms: 2500,
            })
            .collect();

        let samples = curve(kinetics, &given, 0.0, 56.0, 30);
        for pair in samples.windows(2) {
            assert!(pair[1].day >= pair[0].day, "points run forwards in time");
        }
    }

    #[test]
    fn a_peak_is_drawn_at_its_real_height_however_wide_the_window() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let given: Vec<Given> = (0..52)
            .map(|week| Given {
                day: f64::from(week) * 7.0,
                micrograms: 2500,
            })
            .collect();

        let truth = (0..36_400)
            .map(|step| on_board(kinetics, &given, f64::from(step) * 0.01))
            .fold(0.0_f64, f64::max);
        let drawn = curve(kinetics, &given, 0.0, 364.0, 240)
            .iter()
            .map(|sample| sample.micrograms)
            .fold(0.0_f64, f64::max);

        assert!(
            drawn > truth * 0.99,
            "a year's window drew {drawn} for a peak of {truth}"
        );
    }

    // The zero is the literal the function returns at the injection, not a figure it arrived at.
    #[test]
    #[allow(clippy::float_cmp)]
    fn a_dose_reaches_the_circulation_over_hours_rather_than_at_once() {
        let kinetics = Kinetics::of(Drug::Semaglutide);
        let peak = kinetics.after_dose(2500, kinetics.time_to_peak());

        assert_eq!(
            kinetics.after_dose(2500, 0.0),
            0.0,
            "none of it at the needle"
        );
        let after_an_hour = kinetics.after_dose(2500, 1.0 / 24.0);
        assert!(
            after_an_hour < peak * 0.1,
            "an hour in, {after_an_hour} of an eventual {peak} has arrived"
        );
        assert!(
            kinetics.after_dose(2500, 0.5) > peak * 0.4,
            "half a day in, a good part of it has"
        );
    }

    #[test]
    fn the_hour_a_dose_was_given_places_it_within_its_day() {
        let origin = date("2026-08-06");
        let at = |hour: Option<&str>| {
            let dose = Dose {
                id: DoseId::new(0),
                taken: date("2026-08-06"),
                time: hour.map(|text| {
                    NaiveTime::parse_from_str(text, "%H:%M").expect("a time the test wrote")
                }),
                drug: Some(Drug::Semaglutide),
                micrograms: 2500,
                site: Site::LeftAbdomen,
                note: String::new(),
            };
            instant_of(&dose, origin)
        };

        assert!((at(Some("00:00")) - 0.0).abs() < 1e-9);
        assert!((at(Some("06:00")) - 0.25).abs() < 1e-9);
        assert!((at(Some("18:00")) - 0.75).abs() < 1e-9);
        assert!(
            (at(None) - 0.5).abs() < 1e-9,
            "one with no hour sits at midday"
        );
    }

    #[test]
    fn a_dose_given_in_the_evening_peaks_later_than_one_given_that_morning() {
        let kinetics = Kinetics::of(Drug::Tirzepatide);
        let morning = Given {
            day: 0.25,
            micrograms: 5000,
        };
        let evening = Given {
            day: 0.875,
            micrograms: 5000,
        };
        // Tirzepatide peaks about a day out, so mid-morning the next day the earlier injection
        // is past its peak and the later one is still climbing to it.
        let read_at = 1.25;

        assert!(
            on_board(kinetics, &[morning], read_at) > on_board(kinetics, &[evening], read_at),
            "fifteen hours between injections shows in the curve"
        );
    }

    #[test]
    fn the_axis_counts_days_from_its_origin() {
        let origin = date("2026-08-06");
        assert!((day_of(origin, origin) - 0.0).abs() < 1e-9);
        assert!((day_of(date("2026-08-13"), origin) - 7.0).abs() < 1e-9);
        assert!((day_of(date("2026-08-05"), origin) - -1.0).abs() < 1e-9);
    }
}
