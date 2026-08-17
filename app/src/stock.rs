//! The supply record: groups of vials, the ones in use, and the shape all of it takes on the
//! device. [`crate::store`] owns identity, grouping and storage.

use chrono::NaiveDate;

use crate::formulary::Drug;
use crate::units::concentration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Lyophilized,
    Solution { microlitres: u32 },
}

impl Form {
    pub fn label(self) -> &'static str {
        match self {
            Form::Lyophilized => "Lyophilized",
            Form::Solution { .. } => "In solution",
        }
    }

    pub fn microlitres(self) -> Option<u32> {
        match self {
            Form::Lyophilized => None,
            Form::Solution { microlitres } => Some(microlitres),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StockId(pub(crate) u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VialId(pub(crate) u64);

impl StockId {
    #[cfg(test)]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl VialId {
    #[cfg(test)]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Vial {
    pub id: VialId,
    pub micrograms: u32,
    pub microlitres: u32,
    pub opened: NaiveDate,
}

impl Vial {
    pub fn concentration(&self) -> Option<f64> {
        concentration(self.micrograms, self.microlitres)
    }

    pub fn draw(&self, micrograms: u32) -> Option<f64> {
        (self.micrograms > 0).then(|| {
            f64::from(self.microlitres) * f64::from(micrograms) / f64::from(self.micrograms)
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Stock {
    pub id: StockId,
    pub drug: Option<Drug>,
    pub label: String,
    pub micrograms: u32,
    pub form: Form,
    pub sealed: u32,
    pub open: Vec<Vial>,
    pub note: String,
}

impl Stock {
    pub fn name(&self) -> &str {
        let label = self.label.trim();
        match self.drug {
            Some(drug) if label.is_empty() => drug.label(),
            _ if !label.is_empty() => label,
            _ => "Unnamed vial",
        }
    }

    pub fn describes(&self, draft: &StockDraft) -> bool {
        self.drug == draft.drug
            && self.micrograms == draft.micrograms
            && self.form == draft.form
            && self.label.trim().eq_ignore_ascii_case(draft.label.trim())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StockDraft {
    pub drug: Option<Drug>,
    pub label: String,
    pub micrograms: u32,
    pub form: Form,
    pub sealed: u32,
    pub note: String,
}

/// The shape the shelf takes on the device. Frozen: add a `v2` beside `v1` and convert, rather
/// than editing `v1`, which is already on people's devices.
pub mod stored {
    use super::{Form, Stock, StockId, Vial, VialId};

    pub mod v1 {
        use chrono::NaiveDate;
        use serde::{Deserialize, Serialize};

        use crate::formulary::Drug;

        #[derive(Serialize, Deserialize)]
        pub enum Form {
            Lyophilized,
            Solution { microlitres: u32 },
        }

        #[derive(Serialize, Deserialize)]
        pub struct Vial {
            pub id: u64,
            pub micrograms: u32,
            pub microlitres: u32,
            pub opened: NaiveDate,
        }

        #[derive(Serialize, Deserialize)]
        pub struct Stock {
            pub id: u64,
            /// [`Drug`] itself, not a copy: the formulary already treats its variant names as
            /// names on the device.
            pub drug: Option<Drug>,
            pub label: String,
            pub micrograms: u32,
            pub form: Form,
            pub sealed: u32,
            pub open: Vec<Vial>,
            pub note: String,
        }
    }

    impl From<Form> for v1::Form {
        fn from(form: Form) -> Self {
            match form {
                Form::Lyophilized => v1::Form::Lyophilized,
                Form::Solution { microlitres } => v1::Form::Solution { microlitres },
            }
        }
    }

    impl From<v1::Form> for Form {
        fn from(form: v1::Form) -> Self {
            match form {
                v1::Form::Lyophilized => Form::Lyophilized,
                v1::Form::Solution { microlitres } => Form::Solution { microlitres },
            }
        }
    }

    impl From<&Vial> for v1::Vial {
        fn from(vial: &Vial) -> Self {
            v1::Vial {
                id: vial.id.0,
                micrograms: vial.micrograms,
                microlitres: vial.microlitres,
                opened: vial.opened,
            }
        }
    }

    impl From<v1::Vial> for Vial {
        fn from(vial: v1::Vial) -> Self {
            Vial {
                id: VialId(vial.id),
                micrograms: vial.micrograms,
                microlitres: vial.microlitres,
                opened: vial.opened,
            }
        }
    }

    impl From<&Stock> for v1::Stock {
        fn from(entry: &Stock) -> Self {
            v1::Stock {
                id: entry.id.0,
                drug: entry.drug,
                label: entry.label.clone(),
                micrograms: entry.micrograms,
                form: entry.form.into(),
                sealed: entry.sealed,
                open: entry.open.iter().map(v1::Vial::from).collect(),
                note: entry.note.clone(),
            }
        }
    }

    impl From<v1::Stock> for Stock {
        fn from(entry: v1::Stock) -> Self {
            Stock {
                id: StockId(entry.id),
                drug: entry.drug,
                label: entry.label,
                micrograms: entry.micrograms,
                form: entry.form.into(),
                sealed: entry.sealed,
                open: entry.open.into_iter().map(Vial::from).collect(),
                note: entry.note,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{format_concentration, format_units, format_volume};

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date the test wrote")
    }

    fn vial() -> Vial {
        Vial {
            id: VialId::new(0),
            micrograms: 30_000,
            microlitres: 2000,
            opened: date("2026-08-12"),
        }
    }

    #[test]
    fn thirty_milligrams_in_two_millilitres_is_fifteen_per_millilitre() {
        let concentration = vial().concentration().expect("it holds a volume");
        assert!((concentration - 15_000.0).abs() < 1e-9);
        assert_eq!(format_concentration(concentration), "15.0");
    }

    #[test]
    fn a_dose_is_drawn_at_the_volume_holding_it() {
        let vial = vial();
        let drawn = vial.draw(2500).expect("it holds a drug");
        assert!(
            (drawn - 500.0 / 3.0).abs() < 1e-9,
            "2.5 mg is a sixth of it"
        );
        assert_eq!(format_volume(drawn), "0.17");
        assert_eq!(format_units(drawn), "16.7");
    }

    #[test]
    fn drawing_the_whole_vial_is_the_whole_volume() {
        let vial = vial();
        let drawn = vial.draw(vial.micrograms).expect("it holds a drug");
        assert!((drawn - f64::from(vial.microlitres)).abs() < 1e-9);
        assert_eq!(format_units(drawn), "200.0");
    }

    #[test]
    fn a_vial_reads_the_same_concentration_the_mix_that_made_it_was_quoted_at() {
        let vial = vial();
        assert_eq!(
            vial.concentration(),
            concentration(vial.micrograms, vial.microlitres),
            "the preview before mixing and the vial after it are one figure"
        );
    }

    #[test]
    fn a_vial_with_no_volume_has_no_concentration_rather_than_an_infinite_one() {
        let dry = Vial {
            microlitres: 0,
            ..vial()
        };
        assert_eq!(dry.concentration(), None);
        assert_eq!(dry.draw(2500), Some(0.0), "no volume holds the dose");
    }

    #[test]
    fn a_vial_holding_nothing_has_no_draw() {
        let empty = Vial {
            micrograms: 0,
            ..vial()
        };
        assert_eq!(empty.draw(2500), None);
    }

    fn shelf() -> Stock {
        Stock {
            id: StockId::new(3),
            drug: Some(Drug::Tirzepatide),
            label: "Batch B".to_owned(),
            micrograms: 30_000,
            form: Form::Lyophilized,
            sealed: 9,
            open: vec![vial()],
            note: "From the fridge".to_owned(),
        }
    }

    const STORED: &str = concat!(
        r#"{"id":3,"drug":"Tirzepatide","label":"Batch B","micrograms":30000,"#,
        r#""form":"Lyophilized","sealed":9,"#,
        r#""open":[{"id":0,"micrograms":30000,"microlitres":2000,"opened":"2026-08-12"}],"#,
        r#""note":"From the fridge"}"#
    );

    /// Version the schema rather than editing `STORED`: this failing means the shape already on
    /// people's devices has changed.
    #[test]
    fn the_stored_shelf_holds_the_shape_it_was_written_in() {
        let written = serde_json::to_string(&stored::v1::Stock::from(&shelf()))
            .expect("the shelf serialises");
        assert_eq!(written, STORED);
    }

    /// Renaming or removing a variant orphans the vials that chose it; reordering is free.
    #[test]
    fn the_drugs_a_vial_can_hold_keep_the_names_they_are_stored_under() {
        let names: Vec<String> = Drug::ALL
            .iter()
            .map(|drug| serde_json::to_string(drug).expect("a drug serialises"))
            .collect();
        assert_eq!(
            names,
            vec![r#""Semaglutide""#, r#""Tirzepatide""#, r#""Retatrutide""#]
        );
    }

    #[test]
    fn a_shelf_on_the_device_reads_back_as_the_one_that_was_written() {
        let read: stored::v1::Stock = serde_json::from_str(STORED).expect("it reads");
        assert_eq!(Stock::from(read), shelf());
    }

    #[test]
    fn a_vial_that_came_in_solution_stores_the_volume_printed_on_it() {
        let premixed = Stock {
            form: Form::Solution { microlitres: 3000 },
            ..shelf()
        };
        let written = serde_json::to_string(&stored::v1::Stock::from(&premixed))
            .expect("the shelf serialises");
        assert!(
            written.contains(r#""form":{"Solution":{"microlitres":3000}}"#),
            "{written}"
        );

        let read: stored::v1::Stock = serde_json::from_str(&written).expect("it reads");
        assert_eq!(Stock::from(read), premixed);
    }

    #[test]
    fn a_vial_reads_as_the_drug_until_it_is_written_on() {
        let entry = |drug, label: &str| Stock {
            id: StockId::new(0),
            drug,
            label: label.to_owned(),
            micrograms: 30_000,
            form: Form::Lyophilized,
            sealed: 1,
            open: Vec::new(),
            note: String::new(),
        };

        assert_eq!(entry(Some(Drug::Tirzepatide), "").name(), "Tirzepatide");
        assert_eq!(entry(Some(Drug::Tirzepatide), "Batch B").name(), "Batch B");
        assert_eq!(entry(None, "Blend").name(), "Blend");
        assert_eq!(entry(None, "  ").name(), "Unnamed vial");
    }
}
