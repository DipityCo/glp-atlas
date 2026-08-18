//! Reading and writing the figures the app deals in: strengths in milligrams, volumes in
//! millilitres, the draw the two make, and the times of day beside them. Strengths and volumes
//! are carried as whole thousandths.

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

const MAX_MICROGRAMS: u32 = 1_000_000;

const MAX_MICROLITRES: u32 = 100_000;

const MICROLITRES_PER_UNIT: f64 = 10.0;

fn parse_thousandths(text: &str) -> Option<u32> {
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
    whole.checked_mul(1000)?.checked_add(fraction)
}

fn format_thousandths(units: u32) -> String {
    let whole = units / 1000;
    let fraction = units % 1000;
    if fraction == 0 {
        format!("{whole}")
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

/// Takes one unit off the end of a figure as typed, in any case. Once only, so a doubled unit
/// leaves something that is not a figure and is refused.
fn strip_unit<'a>(text: &'a str, unit: &str) -> &'a str {
    let text = text.trim();
    let cut = text.len().saturating_sub(unit.len());
    if text.is_char_boundary(cut) && text[cut..].eq_ignore_ascii_case(unit) {
        text[..cut].trim_end()
    } else {
        text
    }
}

pub fn parse_mg(text: &str) -> Option<u32> {
    let micrograms = parse_thousandths(strip_unit(text, "mg"))?;
    (1..=MAX_MICROGRAMS)
        .contains(&micrograms)
        .then_some(micrograms)
}

pub fn format_mg(micrograms: u32) -> String {
    format_thousandths(micrograms)
}

pub fn parse_ml(text: &str) -> Option<u32> {
    let microlitres = parse_thousandths(strip_unit(text, "ml"))?;
    (1..=MAX_MICROLITRES)
        .contains(&microlitres)
        .then_some(microlitres)
}

pub fn format_ml(microlitres: u32) -> String {
    format_thousandths(microlitres)
}

pub fn format_volume(microlitres: f64) -> String {
    format!("{:.2}", microlitres / 1000.0)
}

pub fn format_units(microlitres: f64) -> String {
    format!("{:.1}", microlitres / MICROLITRES_PER_UNIT)
}

/// Micrograms per millilitre, or `None` for a volume nothing can be drawn out of. The one place
/// this is worked out, so what a mix is said to make and what the vial then reads cannot differ.
pub fn concentration(micrograms: u32, microlitres: u32) -> Option<f64> {
    (microlitres > 0).then(|| f64::from(micrograms) * 1000.0 / f64::from(microlitres))
}

pub fn format_concentration(micrograms_per_ml: f64) -> String {
    format!("{:.1}", micrograms_per_ml / 1000.0)
}

/// A modelled amount in milligrams, at the finest the level model supports reading it: two
/// decimals below a milligram and one above.
pub fn format_level(micrograms: f64) -> String {
    let mg = micrograms / 1000.0;
    if mg >= 1.0 {
        format!("{mg:.1}")
    } else {
        format!("{mg:.2}")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn volumes_survive_being_written_and_read_back() {
        for (text, microlitres) in [("2", 2000), ("0.5", 500), ("1.25", 1250), ("3", 3000)] {
            assert_eq!(parse_ml(text), Some(microlitres), "reading {text}");
            assert_eq!(format_ml(microlitres), text, "writing {text} back");
        }
    }

    #[test]
    fn volumes_are_read_however_they_are_typed() {
        assert_eq!(parse_ml(" 3 mL "), Some(3000));
        assert_eq!(parse_ml("3 ml"), Some(3000), "either case of the unit");
    }

    #[test]
    fn a_volume_that_is_not_a_figure_is_refused() {
        for text in ["", " ", "two", "0", "-1", "1.2345", "1.2.3", "1e3"] {
            assert_eq!(parse_ml(text), None, "`{text}` is not a volume");
        }
    }

    #[test]
    fn the_largest_volume_is_accepted_and_anything_past_it_is_not() {
        assert_eq!(parse_ml("100"), Some(MAX_MICROLITRES));
        assert_eq!(parse_ml("100.001"), None);
    }

    #[test]
    fn each_reader_takes_its_own_unit_once_from_the_end_in_any_case() {
        for (mg, ml) in [("5mg", "5ml"), ("5 MG", "5 ML"), (" 5 Mg ", " 5 mL ")] {
            assert_eq!(parse_mg(mg), Some(5000), "reading {mg}");
            assert_eq!(parse_ml(ml), Some(5000), "reading {ml}");
        }
        for (mg, ml) in [
            ("5mgmg", "5mlml"),
            ("mg5", "ml5"),
            ("5ml", "5mg"),
            ("5m g", "5m l"),
        ] {
            assert_eq!(parse_mg(mg), None, "`{mg}` is not a strength");
            assert_eq!(parse_ml(ml), None, "`{ml}` is not a volume");
        }
    }

    #[test]
    fn a_unit_on_a_figure_that_is_not_ascii_is_left_alone() {
        assert_eq!(
            parse_mg("2·5"),
            None,
            "no slicing through a multi-byte char"
        );
        assert_eq!(parse_ml("2µl"), None);
    }

    #[test]
    fn the_two_units_are_read_and_written_by_the_same_rules() {
        for text in ["2.5", "0.025", "1"] {
            assert_eq!(
                parse_mg(text),
                parse_ml(text),
                "`{text}` reads the same as a strength and as a volume"
            );
        }
    }

    #[test]
    fn a_draw_is_quoted_in_millilitres_and_in_syringe_units() {
        let microlitres = 500.0 / 3.0;
        assert_eq!(format_volume(microlitres), "0.17");
        assert_eq!(format_units(microlitres), "16.7");

        assert_eq!(format_volume(1000.0), "1.00", "a millilitre");
        assert_eq!(format_units(1000.0), "100.0", "is a hundred units");
    }

    #[test]
    fn a_concentration_is_worked_out_and_quoted_in_the_same_unit() {
        // 30 mg made up to 2 mL. The reading and the writing are two halves of one conversion:
        // change the unit one works in and this catches the other.
        let per_ml = concentration(30_000, 2000).expect("it holds a volume");
        assert_eq!(format_concentration(per_ml), "15.0");
        assert_eq!(
            format_concentration(concentration(2500, 1000).unwrap()),
            "2.5"
        );
    }

    #[test]
    fn a_level_sharpens_below_a_milligram_and_rounds_above_it() {
        assert_eq!(format_level(2500.0), "2.5");
        assert_eq!(format_level(1000.0), "1.0");
        assert_eq!(
            format_level(999.0),
            "1.00",
            "the last figure under a milligram"
        );
        assert_eq!(format_level(250.0), "0.25");
        assert_eq!(format_level(0.0), "0.00");
    }

    #[test]
    fn a_volume_of_nothing_has_no_concentration_rather_than_an_infinite_one() {
        assert_eq!(concentration(30_000, 0), None);
        assert_eq!(concentration(0, 2000), Some(0.0), "an empty vial is empty");
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
        assert_eq!(parse_time("07:30:00"), parse_time("07:30"));
    }
}
