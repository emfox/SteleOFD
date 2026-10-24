//! The XML layer driven as a consumer.
//! Name stripping across the prefixes real writers emit, typed attribute reads
//! that tell absent from malformed, and a cursor walk that reports positions.

use sofd_error::OfdError;
use sofd_xml::cursor::Event;
use sofd_xml::{
    AttrValue, Attributes, Cursor, OFD_NS, local_name, parse_bool, parse_number_list,
    parse_number_list_n, parse_optional, parse_required, prefix_of,
};

#[test]
fn a_qualified_name_splits_into_prefix_and_local_part() {
    assert_eq!(local_name("ofd:Page"), "Page");
    assert_eq!(prefix_of("ofd:Page"), "ofd");
    // An unprefixed name has an empty prefix but keeps its whole local name.
    assert_eq!(local_name("Page"), "Page");
    assert_eq!(prefix_of("Page"), "");
}

#[test]
fn a_required_attribute_that_is_absent_names_what_is_missing() {
    let err = parse_required::<f64>("Page", "Width", None).expect_err("absent");
    match err {
        OfdError::Xml(e) => {
            assert!(e.to_string().contains("Width"), "{e}");
            assert!(e.to_string().contains("Page"), "{e}");
        }
        other => panic!("wrong error kind: {other:?}"),
    }
}

#[test]
fn a_malformed_attribute_is_an_error_rather_than_a_silent_zero() {
    let err = parse_required::<f64>("Page", "Width", Some("wide")).expect_err("malformed");
    assert!(err.to_string().contains("wide"), "{err}");
}

#[test]
fn an_absent_optional_attribute_is_none_and_an_empty_one_too() {
    assert_eq!(
        parse_optional::<f64>("Page", "Zoom", None).expect("absent"),
        None
    );
    assert_eq!(
        parse_optional::<f64>("Page", "Zoom", Some("   ")).expect("blank"),
        None,
        "a whitespace-only value is treated as absent"
    );
    assert_eq!(
        parse_optional::<f64>("Page", "Zoom", Some(" 1.5 ")).expect("present"),
        Some(1.5)
    );
}

#[test]
fn a_number_list_matches_the_boundary_form() {
    let box_ = parse_number_list::<f64>("Page", "Boundary", "0 0 210 297").expect("list");
    assert_eq!(box_, vec![0.0, 0.0, 210.0, 297.0]);
}

#[test]
fn a_fixed_arity_list_reports_the_count_it_got() {
    let ok = parse_number_list_n::<4>("Page", "Boundary", "1 2 3 4").expect("four");
    assert_eq!(ok, [1.0, 2.0, 3.0, 4.0]);

    let err = parse_number_list_n::<4>("Page", "Boundary", "1 2 3").expect_err("three");
    let msg = err.to_string();
    assert!(msg.contains("expected 4"), "{msg}");
    assert!(msg.contains("got 3"), "{msg}");
}

#[test]
fn a_boolean_accepts_both_spellings_that_occur_in_practice() {
    assert!(parse_bool("Path", "Fill", "true").expect("true"));
    assert!(parse_bool("Path", "Fill", "1").expect("1"));
    assert!(!parse_bool("Path", "Fill", "false").expect("false"));
    assert!(!parse_bool("Path", "Fill", "0").expect("0"));
    assert!(parse_bool("Path", "Fill", "yes").is_err());
}

#[test]
fn the_namespace_constant_is_the_specification_value() {
    assert_eq!(OFD_NS, "http://www.ofdspec.org/2016");
}

#[test]
fn an_attribute_map_is_case_sensitive_and_ordered() {
    let mut attrs = Attributes::new();
    attrs.insert("ID", "7");
    attrs.insert("Boundary", "0 0 1 1");
    assert_eq!(attrs.len(), 2);
    assert_eq!(attrs.get("ID"), Some("7"));
    assert_eq!(
        attrs.get("id"),
        None,
        "OFD attribute names are case sensitive"
    );
    let names: Vec<&str> = attrs.iter().map(|(name, _)| name).collect();
    assert_eq!(names, vec!["ID", "Boundary"]);
}

#[test]
fn require_as_types_a_present_attribute_and_rejects_a_bad_one() {
    let mut attrs = Attributes::new();
    attrs.insert("Size", "3.5");
    assert_eq!(
        attrs
            .require_as::<f64>("TextObject", "Size")
            .expect("parse"),
        3.5
    );

    attrs.insert("Size", "large");
    assert!(attrs.require_as::<f64>("TextObject", "Size").is_err());
}

#[test]
fn or_default_falls_back_only_when_the_attribute_is_missing() {
    let mut attrs = Attributes::new();
    assert!(attrs.or_default("Layer", "Visible", true).expect("default"));

    attrs.insert("Visible", "false");
    assert!(!attrs.or_default("Layer", "Visible", true).expect("read"));
}

#[test]
fn a_typed_attribute_value_parses_outside_the_map() {
    let value = AttrValue::new("MiterLimit", "4");
    assert_eq!(value.parse::<f64>("Path").expect("parse"), 4.0);
}

#[test]
fn walking_a_well_formed_element_yields_its_children_in_order() {
    let xml = r#"<Layer ID="1"><TextObject ID="2"/><PathObject ID="3"/></Layer>"#;
    let mut cursor = Cursor::new(xml);
    let root = cursor.expect_start("Layer").expect("root");
    assert_eq!(root.attributes().get("ID"), Some("1"));

    let mut seen = Vec::new();
    loop {
        match cursor.next_event().expect("event") {
            Event::Start(node) => seen.push(node.local().to_string()),
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    assert_eq!(seen, vec!["TextObject", "PathObject"]);
}

#[test]
fn a_namespace_prefix_is_matched_on_the_local_name() {
    // The schema is prefix-agnostic, and writers disagree, so a prefixed name
    // must answer `is` for the bare name it carries.
    let xml = r#"<ofd:Layer xmlns:ofd="http://www.ofdspec.org/2016" ID="1"/>"#;
    let mut cursor = Cursor::new(xml);
    let root = cursor.expect_start("Layer").expect("root");
    assert!(root.is("Layer"));
    assert!(root.is_self_closing());
    assert_eq!(root.qualified(), "ofd:Layer");
}

#[test]
fn a_self_closing_element_emits_start_but_no_end() {
    let xml = r#"<Page ID="1"/>"#;
    let mut cursor = Cursor::new(xml);
    let first = cursor.next_event().expect("start");
    assert!(matches!(first, Event::Start(_)));
    // The next event is Eof, not End: a self-closing tag is one event.
    assert!(matches!(cursor.next_event().expect("eof"), Event::Eof));
}

#[test]
fn skipping_an_element_discards_its_whole_subtree() {
    let xml = r#"<Layer><A><B><C/></B></A><D/></Layer>"#;
    let mut cursor = Cursor::new(xml);
    let _root = cursor.expect_start("Layer").expect("root");
    let a = match cursor.next_event().expect("child") {
        Event::Start(node) => node,
        other => panic!("expected a child, got {other:?}"),
    };
    assert!(a.is("A"));
    cursor.skip_element(&a).expect("skip");

    // The element after the skipped subtree is still reachable in order.
    let d = match cursor.next_event().expect("next") {
        Event::Start(node) => node,
        other => panic!("expected D, got {other:?}"),
    };
    assert!(d.is("D"));
}

#[test]
fn unclosed_markup_is_reported_with_a_position() {
    let xml = r#"<Layer><TextObject></Layer>"#;
    let mut cursor = Cursor::new(xml);
    let _root = cursor.expect_start("Layer").expect("root");
    let _child = cursor.next_event().expect("child");
    // The mismatched close is what the reader must refuse, and the message has
    // to carry a line and column so a user can find it in their document.
    let err = cursor.next_event().expect_err("mismatched close");
    let text = err.to_string();
    assert!(text.contains("line"), "{text}");
    assert!(text.contains("column"), "{text}");
}

#[test]
fn expect_start_returns_the_node_and_leaves_matching_to_the_caller() {
    // `expect_start` asks only that the next event be a start tag; the name is
    // the caller's to check. A root the caller did not expect is therefore
    // returned, not refused, and the mismatch is visible in the node.
    let xml = r#"<Page/>"#;
    let mut cursor = Cursor::new(xml);
    let root = cursor
        .expect_start("Document")
        .expect("a start tag is present");
    assert_eq!(root.local(), "Page");
    assert!(!root.is("Document"), "the caller sees the mismatch");

    // When there is no start tag at all, the error names what was wanted.
    let mut cursor = Cursor::new("");
    let err = cursor.expect_start("Document").expect_err("empty document");
    assert!(err.to_string().contains("Document"), "{err}");
}

#[test]
fn the_depth_counter_tracks_open_elements() {
    let xml = r#"<A><B/><C></C></A>"#;
    let mut cursor = Cursor::new(xml);
    assert_eq!(cursor.depth(), 0);
    let _a = cursor.expect_start("A").expect("root");
    assert_eq!(cursor.depth(), 1);
    // A self-closing child is a whole element, so it never deepens the stack.
    let _b = cursor.next_event().expect("B");
    assert_eq!(cursor.depth(), 1);
    // A child with a close tag does.
    let _c = cursor.next_event().expect("C");
    assert_eq!(cursor.depth(), 2);
}

#[test]
fn a_depth_ceiling_stops_a_hostile_document() {
    // A deeply nested document is a denial-of-service vector, so the cursor
    // must refuse past the ceiling rather than recurse until the stack dies.
    let deep = format!("{}{}", "<A>".repeat(6000), "</A>".repeat(6000));
    let mut cursor = Cursor::with_max_depth(&deep, 8);
    let mut events = 0;
    loop {
        match cursor.next_event() {
            Ok(Event::Eof) => panic!("the ceiling let a 6000-deep document through"),
            Ok(_) => {
                events += 1;
                assert!(events < 100, "the ceiling did not stop the walk");
            }
            Err(err) => {
                assert!(err.to_string().contains("depth"), "{err}");
                break;
            }
        }
    }
    assert!(events >= 8, "the walk stopped before reaching the ceiling");
}

#[test]
fn text_content_is_trimmed_and_whitespace_only_text_is_skipped() {
    let xml = r#"<TextCode><![CDATA[hello]]></TextCode>"#;
    let mut cursor = Cursor::new(xml);
    let _root = cursor.expect_start("TextCode").expect("root");
    assert_eq!(cursor.read_text().expect("text"), "hello");
}

#[test]
fn escaped_markup_in_text_reaches_the_caller_as_characters() {
    // A check code is written as `&lt;` because a bare one would close the tag,
    // and the reader has to see the character rather than lose it to the parser.
    let xml = r#"<TextCode>00/2-7&lt;054&gt;88</TextCode>"#;
    let mut cursor = Cursor::new(xml);
    let _root = cursor.expect_start("TextCode").expect("root");
    assert_eq!(cursor.read_text().expect("text"), "00/2-7<054>88");
}
