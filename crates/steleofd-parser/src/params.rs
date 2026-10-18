//! Wiring the drawing parameters a document declares to the objects that name
//! them. Clause 7.9 keeps the table in the `Res` files beside the fonts and
//! the media, which is where every sampled producer puts it.

use std::collections::HashMap;

use steleofd_model::document::{DrawParamList, Page};
use steleofd_model::object::DrawParam;
use steleofd_package::Package;

use crate::content;

/// Drawing parameters keyed by resource id.
pub type ParamTable = HashMap<String, DrawParam>;

/// Drawing parameters, plus the per-page tables folded over them. A single
/// [`load_draw_params`] table is enough for a document with no `PageRes`, so
/// the merge is skipped in that case.
pub fn for_page(
    package: &mut Package,
    doc_dir: &str,
    page: &Page,
    common: &ParamTable,
) -> steleofd_error::Result<ParamTable> {
    let Some(loc) = &page.base_loc else {
        return Ok(common.clone());
    };
    let page_path = steleofd_package::resolve(doc_dir, loc);
    let page_dir = steleofd_package::parent_of(&page_path).to_string();
    let mut merged: Option<ParamTable> = None;
    for name in [steleofd_common::PAGE_RES_XML, steleofd_common::FILE_RES_XML] {
        let path = steleofd_package::join(&page_dir, name);
        let Some(xml) = package.read_optional(&path)? else {
            continue;
        };
        let Ok(text) = String::from_utf8(xml) else {
            continue;
        };
        let table = merged.get_or_insert_with(|| common.clone());
        table.extend(
            content::read_draw_params(&text)?
                .items
                .into_iter()
                .map(|p| (p.id.clone(), p)),
        );
    }
    Ok(merged.unwrap_or_else(|| common.clone()))
}

/// Read the drawing parameters of a document from every table that may hold
/// them, the first declaration of an id winning. `CommonData` is tolerated
/// although the schema puts the table in `Res`; a missing table adds nothing.
pub fn load_draw_params(
    package: &mut Package,
    doc_dir: &str,
    common: Option<&DrawParamList>,
) -> steleofd_error::Result<ParamTable> {
    let mut table = ParamTable::new();
    if let Some(list) = common {
        for param in &list.items {
            table.insert(param.id.clone(), param.clone());
        }
    }
    for name in [
        steleofd_common::DOCUMENT_RES_XML,
        steleofd_common::PUBLIC_RES_XML,
    ] {
        let path = steleofd_package::join(doc_dir, name);
        let Some(xml) = package.read_optional(&path)? else {
            continue;
        };
        let Ok(text) = String::from_utf8(xml) else {
            continue;
        };
        for param in content::read_draw_params(&text)?.items {
            table.entry(param.id.clone()).or_insert(param);
        }
    }
    Ok(table)
}
