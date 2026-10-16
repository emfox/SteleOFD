//! Bare CFF faces: recognition, the CID map and an sfnt wrapper.
//! An OFD package may embed a raw CFF rather than an sfnt and the shaping
//! backend reads sfnt only, so such a font is wrapped in a table directory.

/// sfnt version tag of an OpenType font whose outlines are CFF.
const SFNT_VERSION: u32 = 0x4F54_544F;

/// Magic number every `head` table carries.
const HEAD_MAGIC: u32 = 0x5F0F_3CF5;

/// `maxp` version 0.5, the short form a face without TrueType data may use.
const MAXP_VERSION_HALF: u32 = 0x0000_5000;

/// A bare CFF header is version 1 with a four byte header and an offset size
/// of one to four bytes. Every sfnt magic starts with a zero byte or an
/// ASCII letter, so the first byte alone separates the two families.
pub fn is_bare_cff(data: &[u8]) -> bool {
    matches!(data, [1, _, 4, 1..=4, ..])
}

/// Wrap a bare CFF font in the sfnt directory the shaping backend reads.
/// Only the tables the backend insists on are synthesized; the rest is copied.
pub fn wrap_bare_cff(data: &[u8]) -> Option<Vec<u8>> {
    let top = TopDict::read(data)?;
    let glyphs = Index::read(data, top.char_strings)?.count;
    if glyphs == 0 {
        return None;
    }
    let upem = units_per_em(top.font_matrix[0]);
    let scale = em_scale(top.font_matrix[0]);
    let bbox = [
        clamp_i16(top.font_bbox[0] * scale),
        clamp_i16(top.font_bbox[1] * scale),
        clamp_i16(top.font_bbox[2] * scale),
        clamp_i16(top.font_bbox[3] * scale),
    ];
    let tables: [(&[u8; 4], Vec<u8>); 5] = [
        (b"CFF ", data.to_vec()),
        (b"head", head_table(upem, bbox).to_vec()),
        (b"hhea", hhea_table(upem, bbox[3], bbox[1]).to_vec()),
        (b"hmtx", hmtx_table(upem).to_vec()),
        (b"maxp", maxp_table(glyphs).to_vec()),
    ];
    let mut out = Vec::with_capacity(92 + data.len() + 64);
    out.extend_from_slice(&SFNT_VERSION.to_be_bytes());
    out.extend_from_slice(&(tables.len() as u16).to_be_bytes());
    out.extend_from_slice(&64u16.to_be_bytes());
    out.extend_from_slice(&2u16.to_be_bytes());
    out.extend_from_slice(&16u16.to_be_bytes());
    // The backend binary searches the directory, so the records are sorted
    // by tag and the table data follows in the same order, long aligned.
    let mut offset = 12 + 16 * tables.len();
    for (tag, body) in &tables {
        out.extend_from_slice(*tag);
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        offset = (offset + body.len() + 3) & !3;
    }
    for (_, body) in &tables {
        out.extend_from_slice(body);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    Some(out)
}

/// The CID of every glyph, indexed by glyph id, or `None` when the face is
/// not CID-keyed or names no charset of its own.
pub fn cid_charset(data: &[u8]) -> Option<Vec<u16>> {
    let top = TopDict::read(data)?;
    if !top.cid_keyed {
        return None;
    }
    let offset = top.charset?;
    if offset < 3 {
        return None;
    }
    let glyphs = usize::from(Index::read(data, top.char_strings)?.count);
    if glyphs == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(glyphs);
    out.push(0);
    let format = *data.get(offset)?;
    let mut at = offset + 1;
    match format {
        0 => {
            for _ in 1..glyphs {
                out.push(be_u16(data, at)?);
                at += 2;
            }
        }
        1 | 2 => {
            let wide = format == 2;
            let step = if wide { 2 } else { 1 };
            let mut gid = 1usize;
            while gid < glyphs {
                let first = if wide {
                    usize::from(be_u16(data, at)?)
                } else {
                    usize::from(*data.get(at)?)
                };
                let left = if wide {
                    usize::from(be_u16(data, at + step)?)
                } else {
                    usize::from(*data.get(at + step)?)
                };
                at += 2 * step;
                for k in 0..=left {
                    if gid >= glyphs {
                        break;
                    }
                    out.push(u16::try_from(first + k).unwrap_or(u16::MAX));
                    gid += 1;
                }
            }
        }
        _ => return None,
    }
    Some(out)
}

/// Big endian `u16` at `at`.
fn be_u16(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// One CFF INDEX: where it sits, how many elements it holds and how wide
/// their offsets are.
struct Index<'a> {
    data: &'a [u8],
    at: usize,
    count: u16,
    off_size: u8,
}

impl<'a> Index<'a> {
    /// Read the header of the INDEX starting at `at`.
    fn read(data: &'a [u8], at: usize) -> Option<Self> {
        let count = be_u16(data, at)?;
        if count == 0 {
            // An empty INDEX is its two byte count and nothing else.
            return Some(Self {
                data,
                at,
                count,
                off_size: 0,
            });
        }
        let off_size = *data.get(at + 2)?;
        if !(1..=4).contains(&off_size) {
            return None;
        }
        Some(Self {
            data,
            at,
            count,
            off_size,
        })
    }

    /// Where the offset array begins.
    fn offsets_at(&self) -> usize {
        self.at + if self.count == 0 { 2 } else { 3 }
    }

    /// The `i`-th offset, one based exactly as the spec writes it.
    fn raw_offset(&self, i: u16) -> Option<usize> {
        let start = self.offsets_at() + usize::from(i) * usize::from(self.off_size);
        let bytes = self.data.get(start..start + usize::from(self.off_size))?;
        let mut value = 0usize;
        for b in bytes {
            value = (value << 8) | usize::from(*b);
        }
        Some(value)
    }

    /// The `i`-th element, or `None` past the end.
    fn element(&self, i: u16) -> Option<&'a [u8]> {
        if i >= self.count {
            return None;
        }
        let data_at =
            self.offsets_at() + (usize::from(self.count) + 1) * usize::from(self.off_size);
        let from = data_at + self.raw_offset(i)?.checked_sub(1)?;
        let to = data_at + self.raw_offset(i + 1)?.checked_sub(1)?;
        self.data.get(from..to)
    }

    /// Byte length of the whole INDEX, so the next one can be located.
    fn byte_len(&self) -> Option<usize> {
        if self.count == 0 {
            return Some(2);
        }
        let data_at =
            self.offsets_at() + (usize::from(self.count) + 1) * usize::from(self.off_size);
        let last = self.raw_offset(self.count)?;
        data_at
            .checked_add(last)?
            .checked_sub(1)?
            .checked_sub(self.at)
    }
}

/// A DICT operand: an integer for every field this module reads, since only
/// offsets, counts and the two scale numbers matter.
#[derive(Clone, Copy)]
enum Operand {
    Int(i64),
    Real(f64),
}

impl Operand {
    /// The operand as a number, whichever form it arrived in.
    fn number(self) -> f64 {
        match self {
            Self::Int(v) => v as f64,
            Self::Real(v) => v,
        }
    }

    /// The operand as an offset or a count.
    fn integer(self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(v),
            Self::Real(v) if v.fract() == 0.0 => Some(v as i64),
            Self::Real(_) => None,
        }
    }
}

/// The Top DICT fields the wrapper and the CID map need.
struct TopDict {
    /// Offset of the CharStrings INDEX from the start of the CFF data.
    char_strings: usize,
    /// Offset of the charset, `None` when the DICT names none.
    charset: Option<usize>,
    /// Whether the font is CID-keyed, which is what makes the charset a map
    /// from glyph id to CID rather than a list of glyph names.
    cid_keyed: bool,
    /// Glyph space to em scale, in the six number `FontMatrix` order.
    font_matrix: [f64; 6],
    /// Bounding box of every glyph, in glyph space.
    font_bbox: [f64; 4],
}

impl TopDict {
    /// Walk past the Name INDEX to the Top DICT and read what it declares.
    fn read(data: &[u8]) -> Option<Self> {
        let header_size = usize::from(*data.get(2)?);
        if header_size < 4 {
            return None;
        }
        let name = Index::read(data, header_size)?;
        let top = Index::read(data, header_size + name.byte_len()?)?;
        Self::from_dict(top.element(0)?)
    }

    /// Read the fields this module uses out of one DICT.
    fn from_dict(dict: &[u8]) -> Option<Self> {
        let mut out = Self {
            char_strings: 0,
            charset: None,
            cid_keyed: false,
            font_matrix: [0.001, 0.0, 0.0, 0.001, 0.0, 0.0],
            font_bbox: [0.0; 4],
        };
        let mut stack: Vec<Operand> = Vec::new();
        let mut at = 0;
        while at < dict.len() {
            let b = dict[at];
            match b {
                32..=246 => {
                    stack.push(Operand::Int(i64::from(b) - 139));
                    at += 1;
                }
                247..=250 => {
                    let b1 = i64::from(*dict.get(at + 1)?);
                    stack.push(Operand::Int((i64::from(b) - 247) * 256 + b1 + 108));
                    at += 2;
                }
                251..=254 => {
                    let b1 = i64::from(*dict.get(at + 1)?);
                    stack.push(Operand::Int(-(i64::from(b) - 251) * 256 - b1 - 108));
                    at += 2;
                }
                28 => {
                    let hi = *dict.get(at + 1)?;
                    let lo = *dict.get(at + 2)?;
                    stack.push(Operand::Int(i64::from(i16::from_be_bytes([hi, lo]))));
                    at += 3;
                }
                29 => {
                    let bytes = dict.get(at + 1..at + 5)?;
                    let word: [u8; 4] = bytes.try_into().ok()?;
                    stack.push(Operand::Int(i64::from(i32::from_be_bytes(word))));
                    at += 5;
                }
                30 => {
                    let (value, next) = read_real(dict, at + 1)?;
                    stack.push(Operand::Real(value));
                    at = next;
                }
                12 => {
                    // A two byte operator carries the rest of the fields.
                    let op = *dict.get(at + 1)?;
                    at += 2;
                    match op {
                        6 => out.font_bbox = numbers(&stack)?,
                        7 => out.font_matrix = numbers(&stack)?,
                        30 => out.cid_keyed = true,
                        _ => {}
                    }
                    stack.clear();
                }
                0..=21 => {
                    match b {
                        15 => out.charset = Some(offset_of(&stack)?),
                        17 => out.char_strings = offset_of(&stack)?,
                        _ => {}
                    }
                    stack.clear();
                    at += 1;
                }
                _ => return None,
            }
        }
        Some(out)
    }
}

/// The last `N` operands as numbers, or `None` when the stack is short.
fn numbers<const N: usize>(stack: &[Operand]) -> Option<[f64; N]> {
    if stack.len() < N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, operand) in out.iter_mut().zip(&stack[stack.len() - N..]) {
        *slot = operand.number();
    }
    Some(out)
}

/// The one operand a single value operator names, as an offset.
fn offset_of(stack: &[Operand]) -> Option<usize> {
    usize::try_from(stack.last()?.integer()?).ok()
}

/// Read the nibble encoded real number that starts at `at`, and return the
/// offset just past its terminator.
fn read_real(data: &[u8], at: usize) -> Option<(f64, usize)> {
    let mut text = String::new();
    let mut at = at;
    loop {
        let byte = *data.get(at)?;
        at += 1;
        if !nibble(&mut text, byte >> 4)? {
            break;
        }
        if !nibble(&mut text, byte & 0x0f)? {
            break;
        }
    }
    Some((text.parse().ok()?, at))
}

/// One nibble of a real number; `false` ends it.
fn nibble(text: &mut String, value: u8) -> Option<bool> {
    match value {
        0..=9 => text.push(char::from(b'0' + value)),
        0xa => text.push('.'),
        0xb => text.push('e'),
        0xc => text.push_str("e-"),
        0xe => text.push('-'),
        0xf => return Some(false),
        _ => return None,
    }
    Some(true)
}

/// Units per em the `FontMatrix` implies, inside the range a `head` table
/// may state and outside which the backend refuses the face.
fn units_per_em(matrix_scale: f64) -> u16 {
    let scale = em_scale(matrix_scale);
    if (16.0..=16384.0).contains(&scale) {
        scale as u16
    } else {
        1000
    }
}

/// How many font units one em is, or 1000 when the matrix says nothing.
fn em_scale(matrix_scale: f64) -> f64 {
    if matrix_scale.is_finite() && matrix_scale.abs() > 0.0 {
        (1.0 / matrix_scale).round()
    } else {
        1000.0
    }
}

/// Round and clamp into the `i16` a table field may hold.
fn clamp_i16(value: f64) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    value
        .round()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

/// A `head` table carrying the units per em and the glyph bounding box.
fn head_table(upem: u16, bbox: [i16; 4]) -> [u8; 54] {
    let mut t = [0u8; 54];
    t[0..2].copy_from_slice(&1u16.to_be_bytes());
    t[4..8].copy_from_slice(&0x0001_0000i32.to_be_bytes());
    t[12..16].copy_from_slice(&HEAD_MAGIC.to_be_bytes());
    t[18..20].copy_from_slice(&upem.to_be_bytes());
    t[36..38].copy_from_slice(&bbox[0].to_be_bytes());
    t[38..40].copy_from_slice(&bbox[1].to_be_bytes());
    t[40..42].copy_from_slice(&bbox[2].to_be_bytes());
    t[42..44].copy_from_slice(&bbox[3].to_be_bytes());
    t[46..48].copy_from_slice(&6u16.to_be_bytes());
    t[48..50].copy_from_slice(&2i16.to_be_bytes());
    t
}

/// An `hhea` table. The vertical metrics come from the glyph bounding box,
/// which is the only statement of the em a bare CFF makes.
fn hhea_table(upem: u16, ascender: i16, descender: i16) -> [u8; 36] {
    let mut t = [0u8; 36];
    t[0..2].copy_from_slice(&1u16.to_be_bytes());
    t[4..6].copy_from_slice(&ascender.to_be_bytes());
    t[6..8].copy_from_slice(&descender.to_be_bytes());
    t[10..12].copy_from_slice(&upem.to_be_bytes());
    t[18..20].copy_from_slice(&1i16.to_be_bytes());
    t[34..36].copy_from_slice(&1u16.to_be_bytes());
    t
}

/// One `hmtx` record. A bare CFF states no per glyph advance, so every glyph
/// inherits the last record's, which is the full em.
fn hmtx_table(upem: u16) -> [u8; 4] {
    let mut t = [0u8; 4];
    t[0..2].copy_from_slice(&upem.to_be_bytes());
    t
}

/// A `maxp` table in its short form: the version and the glyph count.
fn maxp_table(glyphs: u16) -> [u8; 6] {
    let mut t = [0u8; 6];
    t[0..4].copy_from_slice(&MAXP_VERSION_HALF.to_be_bytes());
    t[4..6].copy_from_slice(&glyphs.to_be_bytes());
    t
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A Type2 charstring: a 100 unit square, so an outline has real area.
    /// The width leads the first move, which is how Type 2 spells it.
    fn square() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&[239, 139, 139, 21]); // 100 0 0 rmoveto
        v.extend_from_slice(&[239, 139, 5]); // 100 0 rlineto
        v.extend_from_slice(&[139, 239, 5]); // 0 100 rlineto
        v.extend_from_slice(&[28, 0xff, 0x9c, 139, 5]); // -100 0 rlineto
        v.extend_from_slice(&[139, 28, 0xff, 0x9c, 5]); // 0 -100 rlineto
        v.push(14); // endchar
        v
    }

    /// Assemble a CFF INDEX from whole elements.
    fn index(items: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(items.len() as u16).to_be_bytes());
        if items.is_empty() {
            return out;
        }
        // Offsets are one based and count from the byte before the data, so
        // the widest element decides the offset size.
        let total: usize = items.iter().map(|i| i.len()).sum();
        let off_size = if total < usize::from(u16::MAX) { 2 } else { 3 };
        out.push(off_size as u8);
        let mut offset = 1u32;
        for item in items {
            out.extend_from_slice(&offset.to_be_bytes()[4 - off_size..]);
            offset += item.len() as u32;
        }
        out.extend_from_slice(&offset.to_be_bytes()[4 - off_size..]);
        for item in items {
            out.extend_from_slice(item);
        }
        out
    }

    /// Push one DICT integer in its shortest legal form.
    fn push_int(out: &mut Vec<u8>, value: i64) {
        if (-107..=113).contains(&value) {
            out.push((value + 139) as u8);
        } else if (-32768..=32767).contains(&value) {
            out.push(28);
            out.extend_from_slice(&(value as i16).to_be_bytes());
        } else {
            out.push(29);
            out.extend_from_slice(&(value as i32).to_be_bytes());
        }
    }

    /// Encode DICT integer operands.
    fn ints(values: &[i64]) -> Vec<u8> {
        let mut out = Vec::new();
        for v in values {
            push_int(&mut out, *v);
        }
        out
    }

    /// Encode DICT real operands the way the format spells them: the `30`
    /// operator, then one nibble per digit and a terminator at the end.
    fn reals(values: &[f64]) -> Vec<u8> {
        let mut out = Vec::new();
        for v in values {
            let mut nibbles: Vec<u8> = Vec::new();
            for c in format!("{v}").chars() {
                match c {
                    '0'..='9' => nibbles.push(c as u8 - b'0'),
                    '.' => nibbles.push(0xa),
                    '-' => nibbles.push(0xe),
                    _ => {}
                }
            }
            nibbles.push(0xf);
            out.push(30);
            for pair in nibbles.chunks(2) {
                out.push(pair[0] << 4 | pair.get(1).copied().unwrap_or(0xf));
            }
        }
        out
    }

    /// One DICT: operand bytes followed by the operator bytes.
    fn dict(entries: &[(Vec<u8>, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (operands, operator) in entries {
            out.extend_from_slice(operands);
            out.extend_from_slice(operator);
        }
        out
    }

    /// A charset of format 0 naming one CID per glyph past `.notdef`.
    fn charset(cids: &[u16]) -> Vec<u8> {
        let mut out = vec![0u8];
        for cid in cids {
            out.extend_from_slice(&cid.to_be_bytes());
        }
        out
    }

    /// The Top DICT of a bare CFF with three glyphs, two of them named by a
    /// CID. The DICT names offsets into the file it sits in and its own
    /// length moves them, so the layout is walked until it settles.
    pub(crate) fn synthetic_cff() -> Vec<u8> {
        let glyph = square();
        let name = index(&[b"TestCFF"]);
        let strings = index(&[]);
        let subrs = index(&[]);
        let charset = charset(&[963, 1217]);
        let fd_select = vec![0u8, 0, 0];
        let private = dict(&[(ints(&[0]), &[20])]); // defaultWidthX 0
        let charstrings = index(&[&glyph, &glyph, &glyph]);

        let mut top = Vec::new();
        let mut placed = [0usize; 5];
        for _ in 0..4 {
            let fd_array = index(&[&font_dict(private.len(), placed[4])]);
            let mut at = 4 + name.len() + index(&[&top]).len() + strings.len() + subrs.len();
            let mut next = [0usize; 5];
            next[0] = at;
            at += charset.len();
            next[1] = at;
            at += fd_select.len();
            next[2] = at;
            at += fd_array.len();
            next[3] = at;
            at += charstrings.len();
            next[4] = at;
            if next == placed {
                break;
            }
            placed = next;
            top = dict(&[
                (ints(&[0, 0, 0]), &[12, 30]),                          // ROS
                (ints(&[placed[0] as i64]), &[15]),                     // charset
                (ints(&[placed[3] as i64]), &[17]),                     // CharStrings
                (ints(&[placed[2] as i64]), &[12, 36]),                 // FDArray
                (ints(&[placed[1] as i64]), &[12, 37]),                 // FDSelect
                (ints(&[0, 0, 100, 100]), &[12, 6]),                    // FontBBox
                (reals(&[0.001, 0.0, 0.0, 0.001, 0.0, 0.0]), &[12, 7]), // FontMatrix
            ]);
        }
        let fd_array = index(&[&font_dict(private.len(), placed[4])]);
        let mut out = vec![1, 0, 4, 2];
        out.extend_from_slice(&name);
        out.extend_from_slice(&index(&[&top]));
        out.extend_from_slice(&strings);
        out.extend_from_slice(&subrs);
        out.extend_from_slice(&charset);
        out.extend_from_slice(&fd_select);
        out.extend_from_slice(&fd_array);
        out.extend_from_slice(&charstrings);
        out.extend_from_slice(&private);
        out
    }

    /// A Font DICT naming its Private DICT by size and offset.
    fn font_dict(private_size: usize, private_offset: usize) -> Vec<u8> {
        let mut out = Vec::new();
        push_int(&mut out, private_size as i64);
        push_int(&mut out, private_offset as i64);
        out.push(18);
        out
    }

    #[test]
    fn a_bare_cff_header_is_told_apart_from_an_sfnt_magic() {
        assert!(is_bare_cff(&[1, 0, 4, 2]));
        assert!(is_bare_cff(&[1, 0, 4, 3, 0, 1]));
        assert!(!is_bare_cff(&[0, 1, 0, 0]), "sfnt TrueType 1.0");
        assert!(!is_bare_cff(b"true"), "Apple sfnt TrueType");
        assert!(!is_bare_cff(b"OTTO"), "OpenType with CFF outlines");
        assert!(!is_bare_cff(&[1, 0, 5, 2]), "a header past the four bytes");
    }

    #[test]
    fn the_wrapper_hands_the_backend_a_face_it_can_open() {
        let cff = synthetic_cff();
        let sfnt = wrap_bare_cff(&cff).expect("wrap");
        assert_eq!(&sfnt[..4], b"OTTO");
        let face = ttf_parser::Face::parse(&sfnt, 0).expect("the backend opens it");
        assert_eq!(face.number_of_glyphs(), 3);
        assert_eq!(face.units_per_em(), 1000);
        assert!(face.ascender() > 0, "{:?}", face.ascender());
    }

    #[test]
    fn the_charset_names_a_cid_for_every_glyph() {
        let cff = synthetic_cff();
        let cids = cid_charset(&cff).expect("CID keyed");
        assert_eq!(cids, vec![0, 963, 1217]);
    }

    #[test]
    fn a_face_without_ros_has_no_cid_map() {
        let mut cff = synthetic_cff();
        // Drop the ROS entry, leaving the rest of the DICT untouched. The
        // offsets it names are stale after that, but a face that is not
        // CID-keyed has no charset to read, so nothing looks at them.
        let ros = [0x8b, 0x8b, 0x8b, 0x0c, 0x1e];
        let at = cff
            .windows(ros.len())
            .position(|w| w == ros)
            .expect("the ROS entry is in there");
        cff.splice(at..at + ros.len(), []);
        assert!(cid_charset(&cff).is_none());
    }

    #[test]
    fn truncated_or_hostile_bytes_fail_without_panicking() {
        assert!(wrap_bare_cff(&[1, 0, 4, 2]).is_none());
        assert!(wrap_bare_cff(&[]).is_none());
        assert!(cid_charset(&[1, 0, 4, 2]).is_none());
        let mut cff = synthetic_cff();
        for cut in 1..cff.len() {
            cff.truncate(cut);
            let _ = wrap_bare_cff(&cff);
            let _ = cid_charset(&cff);
        }
    }
}
