//! Wiring the drawing parameters a document declares to the objects that name
//! them. Clause 7.9 keeps the table in the `Res` files beside the fonts and
//! the media, which is where every producer in the corpus puts it.

use std::collections::HashMap;

use steleofd_model::document::DrawParamList;
use steleofd_model::object::DrawParam;
use steleofd_package::Package;

use crate::content;

/// Drawing parameters keyed by resource id.
pub type ParamTable = HashMap<String, DrawParam>;

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
