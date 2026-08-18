//! The shapes the records take on the device, and the document they travel in.
//!
//! Nothing is released, so a shape here can still change outright rather than gaining a `v2`
//! beside it; the tests pin what is written so that a change is a deliberate one. A document
//! carrying another [`VERSION`] is left alone rather than read. [`crate::store`] does the reading
//! and writing; this module only says what is read and written.

use serde::{Deserialize, Serialize};

use crate::stock::{Form, Stock, StockId, Vial, VialId};
use crate::store::{Dose, DoseId, Drawn, Medication, Site};
use crate::units::Clock;

/// Bumped when the stored shape changes. A stored document carrying any other version is left
/// untouched rather than read, so a newer build's data survives an older one opening it.
pub const VERSION: u32 = 1;

/// The stored document on the way out.
#[derive(Serialize)]
pub struct Writing<'a> {
    pub version: u32,
    pub doses: Vec<v1::Dose>,
    pub stock: Vec<v1::Stock>,
    pub medication: &'a Medication,
    pub clock: Clock,
}

/// The stored document on the way in. A document this build cannot read whole is left where it
/// is, which is what keeps a newer build's document off an older build's write.
#[derive(Deserialize)]
pub struct Reading {
    pub version: u32,
    pub doses: Vec<v1::Dose>,
    pub stock: Vec<v1::Stock>,
    pub medication: Medication,
    pub clock: Clock,
}

/// The records as this build writes them. Neither has a version of its own: the document
/// carries [`VERSION`] for all of it.
pub mod v1 {
    use chrono::{NaiveDate, NaiveTime};
    use serde::{Deserialize, Serialize};

    use crate::formulary::Drug;

    #[derive(Serialize, Deserialize)]
    pub enum Site {
        LeftAbdomen,
        RightAbdomen,
        LeftThigh,
        RightThigh,
    }

    /// What one vial gave to one dose.
    #[derive(Serialize, Deserialize)]
    pub struct Drawn {
        pub vial: u64,
        pub micrograms: u32,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Dose {
        pub id: u64,
        pub taken: NaiveDate,
        pub time: Option<NaiveTime>,
        /// [`Drug`] itself, not a copy: the formulary already treats its variant names as
        /// names on the device.
        pub drug: Option<Drug>,
        pub micrograms: u32,
        pub site: Site,
        /// The vials it was drawn from, by the ids the shelf stores them under. Empty for a dose
        /// that did not come off the shelf.
        pub from: Vec<Drawn>,
        pub note: String,
    }

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

impl From<Site> for v1::Site {
    fn from(site: Site) -> Self {
        match site {
            Site::LeftAbdomen => v1::Site::LeftAbdomen,
            Site::RightAbdomen => v1::Site::RightAbdomen,
            Site::LeftThigh => v1::Site::LeftThigh,
            Site::RightThigh => v1::Site::RightThigh,
        }
    }
}

impl From<v1::Site> for Site {
    fn from(site: v1::Site) -> Self {
        match site {
            v1::Site::LeftAbdomen => Site::LeftAbdomen,
            v1::Site::RightAbdomen => Site::RightAbdomen,
            v1::Site::LeftThigh => Site::LeftThigh,
            v1::Site::RightThigh => Site::RightThigh,
        }
    }
}

impl From<&Drawn> for v1::Drawn {
    fn from(drawn: &Drawn) -> Self {
        v1::Drawn {
            vial: drawn.vial.0,
            micrograms: drawn.micrograms,
        }
    }
}

impl From<v1::Drawn> for Drawn {
    fn from(drawn: v1::Drawn) -> Self {
        Drawn {
            vial: VialId(drawn.vial),
            micrograms: drawn.micrograms,
        }
    }
}

impl From<&Dose> for v1::Dose {
    fn from(dose: &Dose) -> Self {
        v1::Dose {
            id: dose.id.0,
            taken: dose.taken,
            time: dose.time,
            drug: dose.drug,
            micrograms: dose.micrograms,
            site: dose.site.into(),
            from: dose.from.iter().map(v1::Drawn::from).collect(),
            note: dose.note.clone(),
        }
    }
}

impl From<v1::Dose> for Dose {
    fn from(dose: v1::Dose) -> Self {
        Dose {
            id: DoseId(dose.id),
            taken: dose.taken,
            time: dose.time,
            drug: dose.drug,
            micrograms: dose.micrograms,
            site: dose.site.into(),
            from: dose.from.into_iter().map(Drawn::from).collect(),
            note: dose.note,
        }
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

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveTime};

    use super::*;
    use crate::formulary::Drug;
    use crate::stock::{Form, Stock, StockId, Vial};
    use crate::store::{assumed_time, DoseId};

    fn time(text: &str) -> NaiveTime {
        NaiveTime::parse_from_str(text, "%H:%M").expect("a time the test wrote")
    }

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

    const STORED_SHELF: &str = concat!(
        r#"{"id":3,"drug":"Tirzepatide","label":"Batch B","micrograms":30000,"#,
        r#""form":"Lyophilized","sealed":9,"#,
        r#""open":[{"id":0,"micrograms":30000,"microlitres":2000,"opened":"2026-08-12"}],"#,
        r#""note":"From the fridge"}"#
    );

    /// This failing means the shape written to the device has changed. Nothing is released, so
    /// that is allowed; read the diff and make sure it was meant.
    #[test]
    fn the_stored_shelf_holds_the_shape_it_was_written_in() {
        let written =
            serde_json::to_string(&v1::Stock::from(&shelf())).expect("the shelf serialises");
        assert_eq!(written, STORED_SHELF);
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
        let read: v1::Stock = serde_json::from_str(STORED_SHELF).expect("it reads");
        assert_eq!(Stock::from(read), shelf());
    }

    #[test]
    fn a_vial_that_came_in_solution_stores_the_volume_printed_on_it() {
        let premixed = Stock {
            form: Form::Solution { microlitres: 3000 },
            ..shelf()
        };
        let written =
            serde_json::to_string(&v1::Stock::from(&premixed)).expect("the shelf serialises");
        assert!(
            written.contains(r#""form":{"Solution":{"microlitres":3000}}"#),
            "{written}"
        );

        let read: v1::Stock = serde_json::from_str(&written).expect("it reads");
        assert_eq!(Stock::from(read), premixed);
    }

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
                micrograms: 1500,
            }],
            note: "From the fridge".to_owned(),
        }
    }

    const STORED_DOSE: &str = concat!(
        r#"{"id":3,"taken":"2026-08-06","time":"07:30:00","drug":"Tirzepatide","#,
        r#""micrograms":2500,"site":"LeftThigh","from":[{"vial":4,"micrograms":1500}],"#,
        r#""note":"From the fridge"}"#
    );

    /// This failing means the shape written to the device has changed. Nothing is released, so
    /// that is allowed; read the diff and make sure it was meant.
    #[test]
    fn the_stored_dose_holds_the_shape_it_was_written_in() {
        let written =
            serde_json::to_string(&v1::Dose::from(&logged())).expect("the dose serialises");
        assert_eq!(written, STORED_DOSE);
    }

    #[test]
    fn a_dose_on_the_device_reads_back_as_the_one_that_was_written() {
        let read: v1::Dose = serde_json::from_str(STORED_DOSE).expect("it reads");
        assert_eq!(Dose::from(read), logged());
    }

    /// Renaming or removing a variant orphans the doses that chose it; reordering is free.
    #[test]
    fn the_sites_a_dose_can_name_keep_the_names_they_are_stored_under() {
        let names: Vec<String> = Site::ALL
            .into_iter()
            .map(|site| serde_json::to_string(&v1::Site::from(site)).expect("a site serialises"))
            .collect();
        assert_eq!(
            names,
            vec![
                r#""LeftAbdomen""#,
                r#""RightAbdomen""#,
                r#""LeftThigh""#,
                r#""RightThigh""#
            ]
        );
    }

    #[test]
    fn a_dose_given_at_an_unremembered_hour_is_stored_without_one() {
        let unremembered = Dose {
            time: None,
            ..logged()
        };
        let written =
            serde_json::to_string(&v1::Dose::from(&unremembered)).expect("the dose serialises");
        let read = Dose::from(serde_json::from_str::<v1::Dose>(&written).expect("it reads"));

        assert_eq!(read.time, None);
        assert_eq!(read.hour(), assumed_time());
    }

    #[test]
    fn a_dose_that_did_not_come_off_the_shelf_is_stored_naming_no_vial() {
        let bought = Dose {
            from: Vec::new(),
            ..logged()
        };
        let written = serde_json::to_string(&v1::Dose::from(&bought)).expect("the dose serialises");
        let read = Dose::from(serde_json::from_str::<v1::Dose>(&written).expect("it reads"));

        assert!(read.from.is_empty());
    }

    #[test]
    fn a_stored_document_from_another_version_is_not_read() {
        let payload = serde_json::to_string(&Writing {
            version: 99,
            doses: Vec::new(),
            stock: Vec::new(),
            medication: &Medication::default(),
            clock: Clock::default(),
        })
        .expect("the document serialises");
        let read: Reading = serde_json::from_str(&payload).expect("it is still json");

        assert_ne!(read.version, VERSION);
    }
}
