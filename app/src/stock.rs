//! The supply record: groups of vials and the ones in use. [`crate::store`] owns identity and
//! grouping, [`crate::stored`] the shape all of it takes on the device.

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
