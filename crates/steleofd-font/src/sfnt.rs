//! A structural check before the backend sees the bytes, since a table
//! directory is a document's to choose.

/// The three magic numbers an sfnt may begin with: TrueType 1.0, and the
/// `true` and `OTTO` tags. A bare CFF is wrapped before it reaches this check.
const SFNT_MAGICS: [[u8; 4]; 3] = [[0x00, 0x01, 0x00, 0x00], *b"true", *b"OTTO"];

/// The tag a font collection begins with; each face in it has its own offset.
const COLLECTION_MAGIC: [u8; 4] = *b"ttcf";

/// The index of the first face the file holds, if every directory it names
/// fits. A file this cannot read answers `None`, which the caller reports as
/// a parse error, because that is what a malformed font is.
pub fn first_face(data: &[u8]) -> Option<u32> {
    let magic = data.get(..4)?;
    // Both a single font and a collection answer `0`: the first face of either
    // is the one an id names, and only the collection has a directory to look
    // up. What the check decides is whether every one of those directories fits.
    let fits = if magic == COLLECTION_MAGIC {
        collection_fits(data)
    } else {
        SFNT_MAGICS.iter().any(|m| m == magic) && fits(data, 0)
    };
    fits.then_some(0)
}

/// The offset of each face a collection header lists.
fn face_offsets(data: &[u8]) -> impl Iterator<Item = u32> + '_ {
    let count = data
        .get(8..12)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .unwrap_or(0);
    (0..count).filter_map(move |face| {
        let at = 12 + face * 4;
        let b = data.get(at..at + 4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    })
}

/// A collection header lists an offset per face; every one must fit, because
/// the backend reads whichever face the caller asks for.
fn collection_fits(data: &[u8]) -> bool {
    let Some(header) = data.get(..12) else {
        return false;
    };
    let count = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let Some(end) = count.checked_mul(4).and_then(|n| n.checked_add(12)) else {
        return false;
    };
    if end > data.len() || count == 0 {
        return false;
    }
    face_offsets(data).all(|offset| fits(data, offset as usize))
}

/// Read the directory at `base` and check every record against the file.
fn fits(data: &[u8], base: usize) -> bool {
    let Some(header) = data.get(base..base + 12) else {
        return false;
    };
    let count = u16::from_be_bytes([header[4], header[5]]) as usize;
    // A directory longer than the file cannot be read, and a count this large
    // is not a font; the second bound keeps the offset arithmetic in range.
    let Some(directory_end) = count.checked_mul(16).and_then(|n| n.checked_add(base + 12)) else {
        return false;
    };
    if directory_end > data.len() {
        return false;
    }
    for record in 0..count {
        let start = base + 12 + record * 16;
        let Some(entry) = data.get(start..start + 16) else {
            return false;
        };
        let offset = u32::from_be_bytes([entry[8], entry[9], entry[10], entry[11]]) as u64;
        let length = u32::from_be_bytes([entry[12], entry[13], entry[14], entry[15]]) as u64;
        // The backend asserts that offset plus length stays under 4 GiB, and
        // then reads both from the file; a record that fails here fails there.
        if offset + length > u32::MAX as u64 || offset + length > data.len() as u64 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the table directory describes tables that fit in the file.
    fn directory_fits(data: &[u8]) -> bool {
        first_face(data).is_some()
    }

    /// The smallest directory a check can pass: no records, a valid magic.
    fn empty_directory() -> Vec<u8> {
        let mut bytes = Vec::from([0x00, 0x01, 0x00, 0x00]);
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes.extend_from_slice(&[0u8; 6]);
        bytes
    }

    #[test]
    fn an_empty_directory_fits() {
        assert!(directory_fits(&empty_directory()));
    }

    #[test]
    fn a_record_past_the_end_of_the_file_does_not_fit() {
        let mut bytes = empty_directory();
        bytes[4..6].copy_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(b"head");
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&4096u32.to_be_bytes());
        bytes.extend_from_slice(&54u32.to_be_bytes());
        assert!(!directory_fits(&bytes));
    }

    #[test]
    fn a_record_whose_length_overflows_the_offset_does_not_fit() {
        let mut bytes = empty_directory();
        bytes[4..6].copy_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(b"head");
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&0xffff_ffffu32.to_be_bytes());
        bytes.extend_from_slice(&16u32.to_be_bytes());
        assert!(!directory_fits(&bytes));
    }

    #[test]
    fn a_file_too_short_for_its_directory_does_not_fit() {
        let mut bytes = empty_directory();
        bytes[4..6].copy_from_slice(&8u16.to_be_bytes());
        assert!(!directory_fits(&bytes));
    }

    #[test]
    fn a_collection_whose_face_offset_points_outside_the_file_does_not_fit() {
        let mut bytes = Vec::from(*b"ttcf");
        bytes.extend_from_slice(&[0, 1, 0, 0]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&0xffff_fff0u32.to_be_bytes());
        assert!(!directory_fits(&bytes));
    }

    #[test]
    fn a_collection_pointing_at_an_empty_directory_fits() {
        let mut bytes = Vec::from(*b"ttcf");
        bytes.extend_from_slice(&[0, 1, 0, 0]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&16u32.to_be_bytes());
        bytes.extend_from_slice(&empty_directory());
        assert!(directory_fits(&bytes));
    }

    #[test]
    fn a_magic_that_is_not_sfnt_does_not_fit() {
        assert!(!directory_fits(b"wOFF"));
        assert!(!directory_fits(&[]));
    }
}
