//! The Profile page and the two sub-pages reached from it.
//!
//! The Medication page writes the record the Doses page reads: the drug behind the level curve,
//! the prescribed day, and the titration plan. All of it is the user's own record of what they
//! were prescribed; Atlas states it back and proposes none of it.

use chrono::{NaiveDate, Weekday};
use dioxus::prelude::*;

use super::Row;
use crate::formulary::Drug;
use crate::icons::{ArrowRight, Bell, Check, Cross, Pill};
use crate::nav::SubPage;
use crate::store::{rung, today, Clock, Store, TitrationStep};
use crate::units::{format_mg, parse_mg};

/// The days a weekly dose can fall on, from the start of the week.
const WEEK: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

/// The longest plan a step can run for, in weeks. Five years, past which the entry is a typo.
const MAX_WEEKS: u32 = 260;

/// A weekday in full. Its first three characters label a chip.
fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// How often the drug is taken, as it reads mid-sentence. Weekly is the only schedule the
/// formulary holds; anything else reads by its own interval rather than by a word.
fn cadence(drug: Drug) -> String {
    match drug.interval_days() {
        7 => "weekly".to_owned(),
        days => format!("every {days} days"),
    }
}

/// The plan the entered rows describe, or `None` while one of them is half filled in. A wholly
/// blank row is one being added and counts for nothing.
fn read_plan(rows: &[(String, String)]) -> Option<Vec<TitrationStep>> {
    rows.iter()
        .filter(|(strength, weeks)| !(strength.trim().is_empty() && weeks.trim().is_empty()))
        .map(|(strength, weeks)| {
            Some(TitrationStep {
                micrograms: parse_mg(strength)?,
                weeks: weeks
                    .trim()
                    .parse()
                    .ok()
                    .filter(|weeks| (1..=MAX_WEEKS).contains(weeks))?,
            })
        })
        .collect()
}

fn store_plan(store: &mut Store, rows: &[(String, String)]) {
    let Some(titration) = read_plan(rows) else {
        return;
    };
    write_plan(store, titration);
}

/// Puts the rows that are steps into the record, leaving out any that are not one yet.
///
/// For a row being removed, where waiting for the rest of the form to be valid would drop the
/// removal: the row is gone from the page while the record still holds it, and it comes back the
/// next time the page is opened.
fn store_plan_without(store: &mut Store, rows: &[(String, String)]) {
    write_plan(store, read_steps(rows));
}

fn write_plan(store: &mut Store, titration: Vec<TitrationStep>) {
    let mut medication = store.medication();
    medication.titration = titration;
    store.set_medication(medication);
}

/// The rows that describe a step, skipping the blank and the half-filled.
fn read_steps(rows: &[(String, String)]) -> Vec<TitrationStep> {
    rows.iter()
        .filter_map(|(strength, weeks)| {
            Some(TitrationStep {
                micrograms: parse_mg(strength)?,
                weeks: weeks
                    .trim()
                    .parse()
                    .ok()
                    .filter(|weeks| (1..=MAX_WEEKS).contains(weeks))?,
            })
        })
        .collect()
}

#[component]
pub fn ProfilePage() -> Element {
    let mut store = use_context::<Store>();
    let medication = store.medication();
    let mut units_metric = use_signal(|| false);
    let mut arming = use_signal(|| false);

    let summary = match (medication.drug, medication.dose_day) {
        (Some(drug), Some(day)) => format!("{} · {}s", drug.label(), weekday_name(day)),
        (Some(drug), None) => format!("{} · {}", drug.label(), cadence(drug)),
        (None, _) => "Not set up yet".to_owned(),
    };

    rsx! {
        div { class: "card flush",
            Row {
                icon: rsx! { Pill { size: 20 } },
                title: "Medication",
                sub: "{summary}",
                target: SubPage::Medication,
            }
            Row {
                icon: rsx! { Bell { size: 20 } },
                title: "Reminders",
                sub: "Thursdays at 8:00",
                target: SubPage::Reminders,
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Units" }
            div { class: "chips",
                button {
                    class: if units_metric() { "chip" } else { "chip on" },
                    aria_pressed: "{!units_metric()}",
                    onclick: move |_| units_metric.set(false),
                    "Pounds"
                }
                button {
                    class: if units_metric() { "chip on" } else { "chip" },
                    aria_pressed: "{units_metric()}",
                    onclick: move |_| units_metric.set(true),
                    "Kilograms"
                }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Clock" }
            div { class: "chips",
                for face in Clock::ALL {
                    button {
                        key: "{face:?}",
                        class: if store.clock() == face { "chip on" } else { "chip" },
                        aria_pressed: "{store.clock() == face}",
                        onclick: move |_| store.set_clock(face),
                        "{face.label()}"
                    }
                }
            }
            p { class: "note",
                "The time box on the dose form is drawn by the device and keeps the device's own format."
            }
        }

        div { class: "card flush",
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Export data" }
                    span { class: "row-sub", "CSV of doses and weigh-ins" }
                }
            }
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "About Atlas" }
                    span { class: "row-sub", "Version 0.1.0" }
                }
            }
        }

        button {
            class: if arming() { "btn block danger warn" } else { "btn block danger" },
            aria_describedby: arming().then_some("wipe-warning"),
            onclick: move |_| {
                if arming() {
                    store.wipe();
                    arming.set(false);
                } else {
                    arming.set(true);
                }
            },
            if arming() { "Tap again to delete everything" } else { "Delete all data" }
        }
        if arming() {
            p { id: "wipe-warning", class: "note danger", "This cannot be undone." }
        }

        p { class: "note",
            "Everything stays on this device. Atlas is a log, not medical advice; dose changes belong with your prescriber."
        }
    }
}

#[component]
pub fn MedicationPage() -> Element {
    let mut store = use_context::<Store>();
    let medication = store.medication();
    let today = today();

    let mut rows = use_signal(|| {
        medication
            .titration
            .iter()
            .map(|step| (format_mg(step.micrograms), step.weeks.to_string()))
            .collect::<Vec<(String, String)>>()
    });

    let entered = rows();
    let plan = read_plan(&entered);
    // Only when every row is a step does a row's position match a step's, and only then can the
    // done and current marks land on the right row.
    let aligned = plan.as_ref().filter(|plan| plan.len() == entered.len());
    let started = medication
        .started
        .or_else(|| store.all().last().map(|dose| dose.taken));
    let reached = aligned
        .zip(started)
        .and_then(|(plan, started)| rung(plan, started, today))
        .map(|rung| rung.step);

    let start_text = medication
        .started
        .map_or_else(String::new, |date| date.format("%Y-%m-%d").to_string());

    rsx! {
        div { class: "card tight",
            h2 { class: "card-title", "Medication" }
            div { class: "chips",
                for drug in Drug::ALL {
                    button {
                        class: if medication.drug == Some(drug) { "chip on" } else { "chip" },
                        aria_pressed: "{medication.drug == Some(drug)}",
                        onclick: move |_| {
                            let mut next = store.medication();
                            next.drug = Some(drug);
                            store.set_medication(next);
                        },
                        "{drug.label()}"
                    }
                }
            }
            if let Some(drug) = medication.drug {
                // A drug sold under no name leads with its receptors instead.
                if drug.brands().is_empty() {
                    p { class: "note", "{drug.receptors()} · {cadence(drug)}" }
                } else {
                    p { class: "note",
                        "{drug.brands()} · {drug.receptors()} · {cadence(drug)}"
                    }
                }
                if let Some(caution) = drug.standing().note() {
                    p { class: "note caution", "{caution}" }
                }
            } else {
                p { class: "note",
                    "The level curve on the Doses page is built from this. Each of these clears at its own rate, so the curve is a different shape for each."
                }
            }
        }

        if medication.interval_days() > 1 {
            div { class: "card tight",
                h2 { class: "card-title", "Dose day" }
                div { class: "chips",
                    for day in WEEK {
                        button {
                            class: if medication.dose_day == Some(day) { "chip on" } else { "chip" },
                            aria_label: "{weekday_name(day)}",
                            aria_pressed: "{medication.dose_day == Some(day)}",
                            onclick: move |_| {
                                let mut next = store.medication();
                                next.dose_day = if next.dose_day == Some(day) { None } else { Some(day) };
                                store.set_medication(next);
                            },
                            "{&weekday_name(day)[..3]}"
                        }
                    }
                }
                p { class: "note",
                    "Recorded for reference. The next dose is counted from the last one you logged, not from this day: moving a dose that slipped back onto its old day is a decision for your prescriber."
                }
            }
        }

        div { class: "card",
            div { class: "field",
                label { r#for: "plan-start", "Plan started" }
                input {
                    id: "plan-start",
                    r#type: "date",
                    value: "{start_text}",
                    oninput: move |event| {
                        let mut next = store.medication();
                        next.started = NaiveDate::parse_from_str(&event.value(), "%Y-%m-%d").ok();
                        store.set_medication(next);
                    },
                }
            }
            p { class: "note", "Left empty, the plan is dated from your first logged dose." }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Titration plan" }
            if entered.is_empty() {
                p { class: "note",
                    "Enter the steps your prescriber set and Atlas will say which one you are on."
                }
            }
            for (index , (strength , weeks)) in entered.iter().enumerate() {
                div { key: "{index}", class: "step",
                    div {
                        class: match reached {
                            Some(step) if index + 1 < step => "marker dim",
                            Some(step) if index + 1 == step => "marker",
                            _ => "marker dim",
                        },
                        match reached {
                            Some(step) if index + 1 < step => rsx! { Check { size: 17 } },
                            _ => rsx! { ArrowRight { size: 17 } },
                        }
                    }
                    input {
                        class: "step-figure",
                        r#type: "text",
                        inputmode: "decimal",
                        placeholder: "2.5",
                        aria_label: "Strength at step {index + 1}, in milligrams",
                        value: "{strength}",
                        oninput: move |event| {
                            rows.write()[index].0 = event.value();
                            store_plan(&mut store, &rows());
                        },
                    }
                    span { class: "step-unit", "mg for" }
                    input {
                        class: "step-figure",
                        r#type: "text",
                        inputmode: "numeric",
                        placeholder: "4",
                        aria_label: "Weeks at step {index + 1}",
                        value: "{weeks}",
                        oninput: move |event| {
                            rows.write()[index].1 = event.value();
                            store_plan(&mut store, &rows());
                        },
                    }
                    span { class: "step-unit", "wk" }
                    button {
                        class: "step-drop",
                        aria_label: "Remove step {index + 1}",
                        onclick: move |_| {
                            rows.write().remove(index);
                            store_plan_without(&mut store, &rows());
                        },
                        Cross { size: 16 }
                    }
                }
            }
            if plan.is_none() {
                p { class: "note", "A step needs both a strength and a number of weeks to count." }
            }
        }

        button {
            class: "btn block",
            onclick: move |_| rows.write().push((String::new(), String::new())),
            "Add a step"
        }
    }
}

#[component]
pub fn RemindersPage() -> Element {
    let mut dose_reminder = use_signal(|| true);
    let mut weigh_in = use_signal(|| true);
    let mut refill = use_signal(|| false);
    rsx! {
        div { class: "card flush",
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Dose day" }
                    span { class: "row-sub", "Thursdays at 8:00" }
                }
                button {
                    class: if dose_reminder() { "toggle on" } else { "toggle" },
                    aria_label: "Dose day reminder",
                    aria_pressed: "{dose_reminder()}",
                    onclick: move |_| dose_reminder.set(!dose_reminder()),
                }
            }
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Weigh-in" }
                    span { class: "row-sub", "Wednesdays at 7:00" }
                }
                button {
                    class: if weigh_in() { "toggle on" } else { "toggle" },
                    aria_label: "Weigh-in reminder",
                    aria_pressed: "{weigh_in()}",
                    onclick: move |_| weigh_in.set(!weigh_in()),
                }
            }
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Refill" }
                    span { class: "row-sub", "Five days before the pen runs out" }
                }
                button {
                    class: if refill() { "toggle on" } else { "toggle" },
                    aria_label: "Refill reminder",
                    aria_pressed: "{refill()}",
                    onclick: move |_| refill.set(!refill()),
                }
            }
        }
        p { class: "note", "Reminders are local notifications; nothing leaves the device." }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(strength, weeks)| ((*strength).to_owned(), (*weeks).to_owned()))
            .collect()
    }

    #[test]
    fn a_filled_in_plan_reads_as_its_steps() {
        let plan = read_plan(&rows(&[("0.25", "4"), ("0.5", "4"), ("2.5", "12")]))
            .expect("every row is a step");

        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].micrograms, 250);
        assert_eq!(plan[0].weeks, 4);
        assert_eq!(plan[2].micrograms, 2500);
    }

    #[test]
    fn a_blank_row_is_one_being_added_and_not_a_step() {
        let plan = read_plan(&rows(&[("2.5", "4"), ("", "")])).expect("the blank row is skipped");
        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn a_half_filled_row_leaves_the_plan_unread() {
        assert_eq!(read_plan(&rows(&[("2.5", "")])), None);
        assert_eq!(read_plan(&rows(&[("", "4")])), None);
        assert_eq!(read_plan(&rows(&[("2.5", "4"), ("nonsense", "4")])), None);
    }

    #[test]
    fn a_step_lasts_at_least_a_week_and_not_past_five_years() {
        assert_eq!(read_plan(&rows(&[("2.5", "0")])), None);
        assert_eq!(read_plan(&rows(&[("2.5", "261")])), None);
        assert!(read_plan(&rows(&[("2.5", "260")])).is_some());
    }

    #[test]
    fn an_empty_plan_is_a_plan_with_no_steps() {
        assert_eq!(read_plan(&[]), Some(Vec::new()));
    }

    #[test]
    fn every_weekday_abbreviates_to_three_characters() {
        for day in WEEK {
            let name = weekday_name(day);
            assert!(name.is_char_boundary(3), "`{name}` cannot be cut to three");
            assert_eq!(name.len() > 3, name != &name[..3]);
        }
    }
}
