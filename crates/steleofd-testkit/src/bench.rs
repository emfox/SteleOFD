//! A generated document for the performance baselines. A benchmark needs the
//! same bytes everywhere, and the samples are fetched rather than committed, so
//! the pages are written here; their shape is mixed on purpose.

use steleofd_package::build_archive;

use crate::document::{DOCUMENT_ENTRY, OFD_XML, PUBLIC_RES_XML, font_bytes};

/// Paths, strokes, shadings and text runs one page carries. Divided so a page
/// looks like a document rather than a benchmark: more paths than words, and
/// shading where a scanned invoice puts a seal.
pub const OBJECTS_PER_PAGE: usize = 120;

/// The archive bytes of a document with `pages` pages, each carrying
/// [`OBJECTS_PER_PAGE`] objects. The same bytes come out for the same count.
pub fn bench_archive(pages: usize) -> Vec<u8> {
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        ("OFD.xml".to_string(), OFD_XML.as_bytes().to_vec()),
        (
            DOCUMENT_ENTRY.to_string(),
            bench_document_xml(pages).into_bytes(),
        ),
        (
            "Doc_0/PublicRes.xml".to_string(),
            PUBLIC_RES_XML.as_bytes().to_vec(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf".to_string(), font_bytes()),
    ];
    for page in 0..pages {
        entries.push((
            format!("Doc_0/Pages/Page_{page}/Content.xml"),
            bench_page_xml().into_bytes(),
        ));
    }
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(path, data)| (path.as_str(), data.as_slice()))
        .collect();
    build_archive(&borrowed).expect("benchmark archive")
}

/// The document entry: `pages` page references and one A4 page area.
fn bench_document_xml(pages: usize) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Document>\n  <CommonData>\n    <PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea>\n  </CommonData>\n  <Pages>\n",
    );
    for page in 0..pages {
        xml.push_str(&format!(
            "    <Page ID=\"{id}\" BaseLoc=\"Pages/Page_{page}/Content.xml\"/>\n",
            id = page + 1
        ));
    }
    xml.push_str("  </Pages>\n</Document>");
    xml
}

/// One page of body content, the same on every page of the benchmark.
fn bench_page_xml() -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Page>\n  <Area><PhysicalBox>0 0 210 297</PhysicalBox></Area>\n  <Content>\n    <Layer ID=\"2\" Type=\"Body\">\n",
    );
    for i in 0..OBJECTS_PER_PAGE {
        xml.push_str(&object_xml(i));
        xml.push('\n');
    }
    xml.push_str("    </Layer>\n  </Content>\n</Page>");
    xml
}

/// One object, of a kind chosen by where `i` falls in the page.
fn object_xml(i: usize) -> String {
    let x = 10.0 + (i % 20) as f64 * 9.0;
    let y = 12.0 + (i / 20) as f64 * 13.0;
    let w = 6.0 + (i % 3) as f64;
    let h = 4.0 + (i % 5) as f64;
    let shade = (i % 6) * 20;
    let id = 1000 + i;
    let x2 = x + w;
    let y2 = y + h;
    match i % 6 {
        0 => format!(
            concat!(
                "      <PathObject ID=\"{id}\" Boundary=\"{x} {y} {w} {h}\" Fill=\"true\">\n",
                "        <FillColor Value=\"{r} {g} {b}\"/>\n",
                "        <AbbreviatedData>M {x} {y} L {x2} {y} L {x2} {y2} L {x} {y2} L {x} {y}</AbbreviatedData>\n",
                "      </PathObject>"
            ),
            id = id,
            x = x,
            y = y,
            w = w,
            h = h,
            x2 = x2,
            y2 = y2,
            r = shade + 40,
            g = (shade * 2) % 255,
            b = (shade * 3) % 255
        ),
        1 => format!(
            concat!(
                "      <PathObject ID=\"{id}\" Boundary=\"{x} {y} {w} {h}\" Stroke=\"true\" LineWidth=\"0.4\">\n",
                "        <StrokeColor Value=\"{r} {g} {b}\"/>\n",
                "        <AbbreviatedData>M {x} {y} L {x2} {y2} L {x} {y2} Z</AbbreviatedData>\n",
                "      </PathObject>"
            ),
            id = id,
            x = x,
            y = y,
            w = w,
            h = h,
            x2 = x2,
            y2 = y2,
            r = (shade * 5) % 255,
            g = shade + 60,
            b = 200 - (shade % 180)
        ),
        2 | 3 => format!(
            concat!(
                "      <PathObject ID=\"{id}\" Boundary=\"{x} {y} {w} {h}\" Fill=\"true\">\n",
                "        <FillColor><AxialShd StartPoint=\"{x} {y}\" EndPoint=\"{x2} {y2}\">\n",
                "          <Segment Position=\"0\"><Color Value=\"255 255 255\"/></Segment>\n",
                "          <Segment Position=\"1\"><Color Value=\"20 40 90\"/></Segment>\n",
                "        </AxialShd></FillColor>\n",
                "        <AbbreviatedData>M {x} {y} L {x2} {y} L {x2} {y2} L {x} {y2} L {x} {y}</AbbreviatedData>\n",
                "      </PathObject>"
            ),
            id = id,
            x = x,
            y = y,
            w = w,
            h = h,
            x2 = x2,
            y2 = y2
        ),
        4 => format!(
            concat!(
                "      <PathObject ID=\"{id}\" Boundary=\"{x} {y} {w} {h}\" Fill=\"true\">\n",
                "        <FillColor><RadialShd StartPoint=\"{cx} {cy}\" StartRadius=\"0\" EndPoint=\"{cx} {cy}\" EndRadius=\"{r}\">\n",
                "          <Segment Position=\"0\"><Color Value=\"250 250 210\"/></Segment>\n",
                "          <Segment Position=\"1\"><Color Value=\"160 20 20\"/></Segment>\n",
                "        </RadialShd></FillColor>\n",
                "        <AbbreviatedData>M {x} {y} L {x2} {y} L {x2} {y2} L {x} {y2} L {x} {y}</AbbreviatedData>\n",
                "      </PathObject>"
            ),
            id = id,
            x = x,
            y = y,
            w = w,
            h = h,
            x2 = x2,
            y2 = y2,
            cx = x + w / 2.0,
            cy = y + h / 2.0,
            r = (w.max(h) / 2.0).max(1.0)
        ),
        _ => format!(
            concat!(
                "      <TextObject ID=\"{id}\" Boundary=\"{x} {y} {w} {h}\" Font=\"7\" Size=\"3\">\n",
                "        <FillColor Value=\"10 10 10\"/>\n",
                "        <TextCode X=\"{x}\" Y=\"{ty}\">SteleOFD invoice {n}</TextCode>\n",
                "        <TextCode X=\"{x}\" Y=\"{ty2}\">line {n} of page {p}</TextCode>\n",
                "      </TextObject>"
            ),
            id = id,
            x = x,
            y = y,
            w = w,
            h = h,
            ty = y + h - 1.0,
            ty2 = y + h - 4.0,
            n = i,
            p = i / 20
        ),
    }
}
