//! Property tests for the model layer: a colour written by any producer may be
//! rejected but never panic, and one that parses survives a `Display` round
//! trip back into the same channels.

use proptest::prelude::*;
use sofd_model::ColorRef;

/// A colour spelling built from the shapes a producer might emit.
fn colour_spelling() -> impl Strategy<Value = String> {
    prop_oneof![
        "[0-9A-Fa-f]{6}",
        "#[0-9A-Fa-f]{6}",
        "[0-9A-Fa-f]{3}",
        "#[0-9A-Fa-f]{3}",
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>(), 0u8..5u8).prop_map(
            |(r, g, b, a, kind)| match kind {
                0 => format!("{r} {g} {b}"),
                1 => format!("{r} {g} {b} {a}"),
                2 => format!("{r}, {g}, {b}"),
                3 => format!("{r} {g} {b} ({a})"),
                _ => format!("  {r}   {g}  {b}  "),
            }
        ),
        "\\PC{0,40}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Reading a colour from any string must return a `Result`, never panic.
    /// Colour values come from documents of unknown provenance, so this is the
    /// boundary that keeps malformed input from taking the process down.
    #[test]
    fn reading_a_colour_never_panics(raw in "\\PC{0,80}") {
        let value = ColorRef::from_hex(raw.clone());
        let outcome = std::panic::catch_unwind(|| value.to_rgb().is_ok());
        prop_assert!(outcome.is_ok(), "to_rgb panicked on {raw:?}");
    }

    /// A colour that parses must survive a `Display` and re-read: the value the
    /// printer writes is one the reader still accepts, and the channels are the
    /// same. This is the round trip a writer depends on.
    #[test]
    fn a_parsed_colour_round_trips_through_display(raw in colour_spelling()) {
        let colour = ColorRef::from_hex(raw.clone());
        let Ok((r, g, b)) = colour.to_rgb() else {
            return Ok(());
        };
        let printed = colour.to_string();
        let reparsed = ColorRef::from_hex(printed.clone());
        let (r2, g2, b2) = reparsed
            .to_rgb()
            .map_err(|e| TestCaseError::fail(format!(
                "{raw:?} printed as {printed:?} then failed: {e}"
            )))?;
        prop_assert_eq!((r, g, b), (r2, g2, b2));
    }

    /// An absent colour is fully opaque black, not transparent, so a missing
    /// fill value can never erase a shape.
    #[test]
    fn the_default_colour_is_opaque_black(raw in "\\PC{0,40}") {
        let fallback = ColorRef::default();
        prop_assert_eq!(fallback.to_rgb().expect("black parses"), (0, 0, 0));
        prop_assert_eq!(fallback.alpha(), 255);
        // And an arbitrary value is opaque unless the writer said otherwise.
        let colour = ColorRef::from_hex(raw);
        prop_assert_eq!(colour.alpha(), 255);
    }
}
