//! The Inventory page and the pages reached from it.
//!
//! Everything here reads and writes the supply record in [`crate::store`]. What is left in a
//! vial is not stored; it is read off the dose log.

use chrono::NaiveDate;
use dioxus::prelude::*;

use super::Row;
use crate::formulary::Drug;
use crate::icons::Vial as VialIcon;
use crate::nav::{Nav, SubPage};
use crate::stock::{Form, Stock, StockDraft, StockId, Vial, VialId};
use crate::store::{today, Dose, Store};
use crate::units::{
    concentration, format_concentration, format_mg, format_ml, format_units, format_volume,
    parse_mg, parse_ml,
};

fn short_date(date: NaiveDate) -> String {
    date.format("%-d %b").to_string()
}

/// A named drug quotes its own doses or none: a strength given for one drug was never given for
/// another, and a draw computed from it would name a dose that never happened.
fn last_strength(log: &[Dose], drug: Option<Drug>) -> Option<u32> {
    let last = match drug {
        Some(drug) => log.iter().find(|dose| dose.drug == Some(drug)),
        None => log.first(),
    };
    last.map(|dose| dose.micrograms)
}

fn counts(sealed: u32, open: usize) -> String {
    match (sealed, open) {
        (0, 0) => "None left".to_owned(),
        (0, open) => format!("{open} open"),
        (sealed, 0) => format!("{sealed} sealed"),
        (sealed, open) => format!("{sealed} sealed · {open} open"),
    }
}

pub fn vial_label(store: Store, entry: &Stock, vial: &Vial) -> String {
    format!(
        "{} · {} mg left",
        entry.name(),
        format_mg(store.remaining(vial))
    )
}

#[component]
fn MissingStock() -> Element {
    rsx! {
        div { class: "card",
            h2 { class: "card-title", "Not in the inventory" }
            p { class: "note", "These vials are no longer on the shelf." }
        }
    }
}

#[component]
pub fn InventoryPage() -> Element {
    let mut nav = use_context::<Nav>();
    let store = use_context::<Store>();
    let shelf = store.stock();

    rsx! {
        if shelf.is_empty() {
            div { class: "card",
                h2 { class: "card-title", "Nothing on the shelf" }
                p { class: "note",
                    "Enter the vials you have and Atlas will count them, mix them, and take a dose out of one when you say so."
                }
            }
        } else {
            div { class: "card flush",
                for entry in shelf.iter() {
                    Row {
                        key: "{entry.id:?}",
                        icon: rsx! { VialIcon { size: 20 } },
                        title: "{entry.name()}",
                        sub: "{format_mg(entry.micrograms)} mg · {entry.form.label()} · {counts(entry.sealed, entry.open.len())}",
                        meta: "{format_mg(store.held(entry))} mg",
                        target: SubPage::StockDetail(entry.id),
                    }
                }
            }
        }

        button {
            class: "btn primary block",
            onclick: move |_| nav.push(SubPage::AddStock(None)),
            if shelf.is_empty() { "Add your first vials" } else { "Add vials" }
        }

        p { class: "note",
            "The inventory is kept beside the dose log, not inside it. A dose can be logged whether or not it came out of a vial here."
        }
    }
}

/// Takes a sealed vial out of a group and puts it in use, at the volume it is made up to.
/// `onmixed` carries the new vial, so a caller composing a dose can draw from it at once.
#[component]
pub fn MixControl(entry: Stock, onmixed: EventHandler<VialId>) -> Element {
    let mut store = use_context::<Store>();
    let mut volume = use_signal(String::new);

    let id = entry.id;
    let fixed = entry.form.microlitres();
    let mixing = fixed.or_else(|| parse_ml(&volume()));
    let mistyped = fixed.is_none() && mixing.is_none() && !volume().trim().is_empty();
    let would_be = mixing.filter(|_| fixed.is_none()).and_then(|microlitres| {
        let per_ml = concentration(entry.micrograms, microlitres)?;
        Some(format!(
            "{} mg in {} mL is {} mg/mL.",
            format_mg(entry.micrograms),
            format_ml(microlitres),
            format_concentration(per_ml)
        ))
    });
    let powder = entry.form == Form::Lyophilized;

    rsx! {
        if let Some(microlitres) = fixed {
            p { class: "note", "Each of these holds {format_ml(microlitres)} mL as it came." }
        } else {
            div { class: "field",
                label { r#for: "mix-volume", "Diluent in mL" }
                input {
                    id: "mix-volume",
                    r#type: "text",
                    inputmode: "decimal",
                    placeholder: "2",
                    value: "{volume}",
                    aria_describedby: "mix-volume-note mix-volume-makes",
                    oninput: move |event| volume.set(event.value()),
                }
            }
            if mistyped {
                p { id: "mix-volume-note", class: "note",
                    "A volume is a number of millilitres, such as 2."
                }
            }
            if let Some(would_be) = would_be {
                p { id: "mix-volume-makes", class: "note",
                    "{would_be} How much diluent to use is yours to decide."
                }
            }
        }
        button {
            class: "btn block",
            disabled: mixing.is_none(),
            onclick: move |_| {
                if let Some(vial) = mixing.and_then(|ml| store.open_vial(id, ml, today())) {
                    onmixed.call(vial);
                }
            },
            if powder { "Mix one vial" } else { "Open one vial" }
        }
    }
}

#[component]
pub fn StockDetailPage(id: StockId) -> Element {
    let mut nav = use_context::<Nav>();
    let mut store = use_context::<Store>();
    let mut arming = use_signal(|| false);
    let discarding = use_signal(|| None::<VialId>);

    let Some(entry) = store.stock_of(id) else {
        return rsx! { MissingStock {} };
    };

    let powder = entry.form == Form::Lyophilized;
    let strength = last_strength(&store.all(), entry.drug);

    rsx! {
        div { class: "card hero",
            div { class: "hero-main",
                span { class: "hero-value",
                    "{format_mg(entry.micrograms)}"
                    span { class: "hero-unit", "mg per vial" }
                }
                span { class: "hero-label", "{entry.name()} · {entry.form.label()}" }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "On the shelf" }
            div { class: "stats",
                div { class: "stat",
                    span { class: "stat-value", "{entry.sealed}" }
                    span { class: "stat-label", "Sealed" }
                }
                div { class: "stat",
                    span { class: "stat-value", "{entry.open.len()}" }
                    span { class: "stat-label", "In use" }
                }
                div { class: "stat",
                    span { class: "stat-value", "{format_mg(store.held(&entry))}" }
                    span { class: "stat-label", "mg on hand" }
                }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", if powder { "Mix a vial" } else { "Open a vial" } }
            if entry.sealed == 0 {
                p { class: "note", "Nothing sealed is left in this group." }
            } else {
                MixControl { entry: entry.clone(), onmixed: move |_| {} }
            }
        }

        if !entry.open.is_empty() {
            div { class: "card tight",
                h2 { class: "card-title", "In use" }
                for vial in entry.open.iter() {
                    OpenVial {
                        key: "{vial.id:?}",
                        stock: id,
                        vial: vial.clone(),
                        strength,
                        arming: discarding,
                    }
                }
            }
        }

        if !entry.note.is_empty() {
            div { class: "card",
                h2 { class: "card-title", "Notes" }
                p { class: "note", "{entry.note}" }
            }
        }

        button {
            class: "btn block",
            onclick: move |_| nav.push(SubPage::EditStock(id)),
            "Edit these vials"
        }
        button {
            class: if arming() { "btn block warn" } else { "btn block" },
            aria_describedby: arming().then_some("stock-delete-warning"),
            onclick: move |_| {
                if arming() {
                    store.remove_stock(id);
                    nav.back();
                } else {
                    arming.set(true);
                }
            },
            if arming() { "Tap again to delete" } else { "Delete these vials" }
        }
        if arming() {
            p { id: "stock-delete-warning", class: "note caution",
                "Doses drawn from these vials stay in the log; they simply stop naming a vial."
            }
        }
    }
}

#[component]
fn OpenVial(
    stock: StockId,
    vial: Vial,
    strength: Option<u32>,
    mut arming: Signal<Option<VialId>>,
) -> Element {
    let mut store = use_context::<Store>();
    let armed = arming() == Some(vial.id);
    let left = store.remaining(&vial);

    let left_volume = vial
        .draw(left)
        .filter(|microlitres| *microlitres > 0.0)
        .map_or_else(String::new, |microlitres| {
            format!(" ≈ {} mL", format_volume(microlitres))
        });
    let concentration = vial.concentration().map_or_else(String::new, |per_ml| {
        format!(" · {} mg/mL", format_concentration(per_ml))
    });
    let draw = strength.and_then(|micrograms| {
        vial.draw(micrograms)
            .filter(|microlitres| *microlitres > 0.0)
            .map(|microlitres| {
                format!(
                    "A {} mg dose is {} mL, {} units",
                    format_mg(micrograms),
                    format_volume(microlitres),
                    format_units(microlitres)
                )
            })
    });

    rsx! {
        div { class: "step",
            div { class: "marker dim", VialIcon { size: 17 } }
            div { class: "row-main",
                span { class: "row-title", "{format_mg(left)} mg left{left_volume}" }
                span { class: "row-sub",
                    "Opened {short_date(vial.opened)} at {format_ml(vial.microlitres)} mL{concentration}"
                }
                if let Some(draw) = draw {
                    span { class: "row-sub", "{draw}" }
                }
            }
            button {
                class: if armed { "chip warn" } else { "chip" },
                aria_label: if armed {
                    format!("Discard? Tap again to discard the vial opened {}", short_date(vial.opened))
                } else {
                    format!("Discard the vial opened {}", short_date(vial.opened))
                },
                onclick: move |_| {
                    if armed {
                        store.discard_vial(stock, vial.id);
                        arming.set(None);
                    } else {
                        arming.set(Some(vial.id));
                    }
                },
                if armed { "Discard?" } else { "Discard" }
            }
        }
    }
}

#[component]
pub fn AddStockPage(drug: Option<Drug>) -> Element {
    rsx! {
        StockForm { opening: drug }
    }
}

#[component]
pub fn EditStockPage(id: StockId) -> Element {
    let store = use_context::<Store>();
    if store.stock_of(id).is_none() {
        return rsx! { MissingStock {} };
    }
    rsx! {
        StockForm { id }
    }
}

#[component]
fn StockForm(id: Option<StockId>, opening: Option<Drug>) -> Element {
    let mut nav = use_context::<Nav>();
    let mut store = use_context::<Store>();

    let existing = id.and_then(|id| store.stock_of(id));

    let default_drug = existing.as_ref().map_or_else(
        || opening.or_else(|| store.medication().drug),
        |entry| entry.drug,
    );
    let default_label = existing
        .as_ref()
        .map_or_else(String::new, |entry| entry.label.clone());
    let default_strength = existing
        .as_ref()
        .map_or_else(String::new, |entry| format_mg(entry.micrograms));
    let default_solution = existing
        .as_ref()
        .is_some_and(|entry| entry.form != Form::Lyophilized);
    let default_volume = existing
        .as_ref()
        .and_then(|entry| entry.form.microlitres())
        .map(format_ml)
        .unwrap_or_default();
    let default_count = existing
        .as_ref()
        .map_or(1, |entry| entry.sealed)
        .to_string();
    let default_note = existing
        .as_ref()
        .map_or_else(String::new, |entry| entry.note.clone());
    let in_use = existing.as_ref().map_or(0, |entry| entry.open.len());

    let mut drug = use_signal(move || default_drug);
    let mut label = use_signal(move || default_label);
    let mut strength = use_signal(move || default_strength);
    let mut solution = use_signal(move || default_solution);
    let mut volume = use_signal(move || default_volume);
    let mut count = use_signal(move || default_count);
    let mut note = use_signal(move || default_note);

    let micrograms = parse_mg(&strength());
    let mistyped = micrograms.is_none() && !strength().trim().is_empty();
    let microlitres = parse_ml(&volume());
    let miscast = solution() && microlitres.is_none() && !volume().trim().is_empty();
    let form = if solution() {
        microlitres.map(|microlitres| Form::Solution { microlitres })
    } else {
        Some(Form::Lyophilized)
    };
    let sealed: Option<u32> = count().trim().parse().ok();
    let miscounted = sealed.is_none() && !count().trim().is_empty();
    // Zero is a real edit, the sealed vials being used up, but not a real addition.
    let counted = sealed.is_some_and(|sealed| id.is_some() || sealed > 0);
    let ready = micrograms.is_some() && form.is_some() && counted;

    rsx! {
        div { class: "card tight",
            h2 { class: "card-title", "Drug" }
            div { class: "chips",
                for option in Drug::ALL {
                    button {
                        class: if drug() == Some(option) { "chip on" } else { "chip" },
                        aria_pressed: "{drug() == Some(option)}",
                        onclick: move |_| drug.set((drug() != Some(option)).then_some(option)),
                        "{option.label()}"
                    }
                }
            }
            div { class: "field",
                label { r#for: "stock-label", "What the label says" }
                input {
                    id: "stock-label",
                    r#type: "text",
                    placeholder: if drug().is_some() { "Optional" } else { "A blend, or a compounded mix" },
                    value: "{label}",
                    oninput: move |event| label.set(event.value()),
                }
            }
            if drug().is_none() && label().trim().is_empty() {
                p { class: "note",
                    "Without a drug or a label these read as unnamed vials. A dose drawn from one still records whichever drug is on the dose."
                }
            }
        }

        div { class: "card",
            div { class: "pair",
                div { class: "field",
                    label { r#for: "stock-strength", "mg per vial" }
                    input {
                        id: "stock-strength",
                        r#type: "text",
                        inputmode: "decimal",
                        placeholder: "30",
                        value: "{strength}",
                        aria_describedby: "stock-strength-note",
                        oninput: move |event| strength.set(event.value()),
                    }
                }
                div { class: "field",
                    label { r#for: "stock-count", "Sealed vials" }
                    input {
                        id: "stock-count",
                        r#type: "text",
                        inputmode: "numeric",
                        placeholder: "10",
                        value: "{count}",
                        aria_describedby: "stock-count-note",
                        oninput: move |event| count.set(event.value()),
                    }
                }
            }
            if mistyped {
                p { id: "stock-strength-note", class: "note",
                    "A strength is a number of milligrams, such as 30."
                }
            }
            if miscounted {
                p { id: "stock-count-note", class: "note", "A count is a whole number of vials." }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "How they came" }
            div { class: "chips",
                button {
                    class: if solution() { "chip" } else { "chip on" },
                    aria_pressed: "{!solution()}",
                    onclick: move |_| solution.set(false),
                    "Lyophilized"
                }
                button {
                    class: if solution() { "chip on" } else { "chip" },
                    aria_pressed: "{solution()}",
                    onclick: move |_| solution.set(true),
                    "In solution"
                }
            }
            if solution() {
                div { class: "field",
                    label { r#for: "stock-volume", "mL per vial" }
                    input {
                        id: "stock-volume",
                        r#type: "text",
                        inputmode: "decimal",
                        placeholder: "3",
                        value: "{volume}",
                        aria_describedby: "stock-volume-note",
                        oninput: move |event| volume.set(event.value()),
                    }
                }
                if miscast {
                    p { id: "stock-volume-note", class: "note",
                        "A volume is a number of millilitres, such as 3."
                    }
                }
            } else {
                p { class: "note",
                    "Powder. Nothing is drawn from one until it is mixed, and the volume is chosen then."
                }
            }
        }

        div { class: "card",
            div { class: "field",
                label { r#for: "stock-notes", "Notes" }
                textarea {
                    id: "stock-notes",
                    rows: "2",
                    placeholder: "Batch, supplier, expiry",
                    value: "{note}",
                    oninput: move |event| note.set(event.value()),
                }
            }
        }

        if id.is_none() {
            p { class: "note",
                "Vials matching a group already on the shelf are counted into it, so ten of one thing stay one entry."
            }
        } else if in_use == 1 {
            p { class: "note", "The vial already in use keeps what it was opened holding." }
        } else if in_use > 1 {
            p { class: "note",
                "The {in_use} vials already in use keep what they were opened holding."
            }
        }

        button {
            class: "btn primary block",
            disabled: !ready,
            onclick: move |_| {
                let (Some(micrograms), Some(form), Some(sealed)) = (micrograms, form, sealed) else {
                    return;
                };
                let draft = StockDraft {
                    drug: drug(),
                    label: label().trim().to_owned(),
                    micrograms,
                    form,
                    sealed,
                    note: note().trim().to_owned(),
                };
                match id {
                    Some(id) => store.update_stock(id, draft),
                    None => {
                        store.add_stock(draft);
                    }
                }
                nav.back();
            },
            if id.is_some() { "Save changes" } else { "Add to inventory" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::store::DoseId;

    fn dose(id: u64, drug: Option<Drug>, micrograms: u32) -> Dose {
        Dose {
            id: DoseId::new(id),
            taken: NaiveDate::from_ymd_opt(2026, 8, 6).expect("a date the test wrote"),
            time: None,
            drug,
            micrograms,
            site: crate::store::Site::LeftThigh,
            from: Vec::new(),
            note: String::new(),
        }
    }

    #[test]
    fn a_named_drug_is_quoted_at_its_own_doses_and_at_no_others() {
        let log = [
            dose(2, Some(Drug::Semaglutide), 1000),
            dose(1, Some(Drug::Tirzepatide), 2500),
        ];

        assert_eq!(last_strength(&log, Some(Drug::Tirzepatide)), Some(2500));
        assert_eq!(
            last_strength(&log, Some(Drug::Retatrutide)),
            None,
            "a drug never logged borrows no other drug's strength"
        );
        assert_eq!(
            last_strength(&log, None),
            Some(1000),
            "a vial naming no drug takes the last dose logged"
        );
    }

    #[test]
    fn a_strength_needs_a_dose_to_come_from() {
        assert_eq!(last_strength(&[], Some(Drug::Tirzepatide)), None);
        assert_eq!(last_strength(&[], None), None);
    }

    #[test]
    fn a_group_reads_its_two_counts_and_says_so_when_it_has_neither() {
        assert_eq!(counts(9, 1), "9 sealed · 1 open");
        assert_eq!(counts(9, 0), "9 sealed");
        assert_eq!(counts(0, 2), "2 open");
        assert_eq!(counts(0, 0), "None left");
    }
}
