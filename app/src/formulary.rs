//! What the app knows about each drug it can track: properties of the molecule, the same for
//! everyone. Nothing here is read from or written to the device; [`crate::store`] holds all of
//! that.
//!
//! UNVERIFIED. The figures are approximate, not read off prescribing information, and no test
//! can tell a wrong one from a right one: the tests check the arithmetic built on them and take
//! them as given. Check each against its source before a release states it as fact.

use serde::{Deserialize, Serialize};

/// Whether a drug has a label behind it. A licensed one's figures can be checked against one and
/// its dosing is the schedule it was approved on; an investigational one has neither, so its
/// figures here are softer than the rest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Standing {
    Licensed,
    Investigational,
}

impl Standing {
    pub fn note(self) -> Option<&'static str> {
        match self {
            Standing::Licensed => None,
            Standing::Investigational => Some(
                "Not approved by any regulator. The figures behind this curve come from trials \
                 rather than a label, so it is a rougher estimate than the rest of the app.",
            ),
        }
    }
}

/// Serialized by variant name, so a name here is a name on the device: adding one is free, and
/// renaming or removing one orphans the records that chose it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Drug {
    Semaglutide,
    Tirzepatide,
    Retatrutide,
}

impl Drug {
    pub const ALL: [Drug; 3] = [Drug::Semaglutide, Drug::Tirzepatide, Drug::Retatrutide];

    pub fn label(self) -> &'static str {
        match self {
            Drug::Semaglutide => "Semaglutide",
            Drug::Tirzepatide => "Tirzepatide",
            Drug::Retatrutide => "Retatrutide",
        }
    }

    /// What the pen is likely to say.
    pub fn brands(self) -> &'static str {
        match self {
            Drug::Semaglutide => "Ozempic, Wegovy",
            Drug::Tirzepatide => "Mounjaro, Zepbound",
            Drug::Retatrutide => "",
        }
    }

    pub fn receptors(self) -> &'static str {
        match self {
            Drug::Semaglutide => "GLP-1",
            Drug::Tirzepatide => "GLP-1 and GIP",
            Drug::Retatrutide => "GLP-1, GIP and glucagon",
        }
    }

    /// The custom property the stylesheet paints this drug's curve with. Fixed to the drug rather
    /// than to its place in a log, so a curve keeps its colour as others come and go.
    pub fn tint(self) -> &'static str {
        match self {
            Drug::Semaglutide => "--tint-semaglutide",
            Drug::Tirzepatide => "--tint-tirzepatide",
            Drug::Retatrutide => "--tint-retatrutide",
        }
    }

    pub fn standing(self) -> Standing {
        match self {
            Drug::Semaglutide | Drug::Tirzepatide => Standing::Licensed,
            Drug::Retatrutide => Standing::Investigational,
        }
    }

    /// Spelled out per drug even where they agree, since they move independently.
    #[allow(clippy::match_same_arms)]
    pub fn interval_days(self) -> u32 {
        match self {
            Drug::Semaglutide => 7,
            Drug::Tirzepatide => 7,
            Drug::Retatrutide => 7,
        }
    }

    /// Terminal half-life.
    pub fn elimination_half_life_days(self) -> f64 {
        match self {
            Drug::Semaglutide => 7.0,
            Drug::Tirzepatide => 5.0, // about 117 hours
            Drug::Retatrutide => 6.0,
        }
    }

    /// The less settled of the two constants: each is a point picked inside the range beside it.
    #[allow(clippy::match_same_arms)]
    pub fn time_to_peak_days(self) -> f64 {
        match self {
            Drug::Semaglutide => 2.0, // 1 to 3 days
            Drug::Tirzepatide => 1.0, // median near 24 hours, over a wide range
            Drug::Retatrutide => 1.0, // roughly a day, and the least certain of these
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drug_says_what_it_is() {
        for drug in Drug::ALL {
            assert!(!drug.label().is_empty(), "{drug:?} has no name");
            assert!(!drug.receptors().is_empty(), "{drug:?} acts on nothing");
            assert_eq!(
                drug.brands().is_empty(),
                drug.standing() == Standing::Investigational,
                "{drug:?} is sold under a name exactly when it is licensed"
            );
        }
    }

    #[test]
    fn every_drug_carries_figures_the_model_can_run_on() {
        for drug in Drug::ALL {
            assert!(
                drug.elimination_half_life_days() > 0.0,
                "{drug:?} is cleared instantly"
            );
            assert!(
                drug.time_to_peak_days() > 0.0,
                "{drug:?} peaks at the needle"
            );
            assert!(drug.interval_days() > 0, "{drug:?} is given continuously");
            // A peak at or past `1 / ke` is outside the model's reach: no absorption rate
            // produces it, and the solver runs to its floor instead.
            assert!(
                drug.time_to_peak_days()
                    < drug.elimination_half_life_days() / std::f64::consts::LN_2,
                "{drug:?} peaks later than any absorption rate can put it"
            );
            assert!(
                drug.time_to_peak_days() < f64::from(drug.interval_days()),
                "{drug:?} peaks after its next dose is due"
            );
        }
    }

    #[test]
    fn a_drug_without_a_label_behind_it_says_so() {
        assert_eq!(Drug::Semaglutide.standing().note(), None);
        assert!(Drug::Retatrutide.standing().note().is_some());
    }
}
