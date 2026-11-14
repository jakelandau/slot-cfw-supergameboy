use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TITLE_OFF: u64 = 0x134;
const TITLE_LEN: usize = 11;

const CGB_OFF: u64 = 0x143;
/// `0x03` here says the cart supports Super Game Boy functions. SameBoy reads exactly these two
/// bytes to decide whether to boot the cart as a Super Game Boy, so slot asks the same question.
const SGB_OFF: u64 = 0x146;

const CODE_OFF: usize = 0x13f;
const DEST_OFF: usize = 0x14a;
pub const HEADER_LEN: usize = 0x150;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub title: String,
    pub code: String,
    pub cgb: u8,
    pub japan: bool,
    /// The cart draws Super Game Boy borders and palettes when run on that hardware. Only a cart
    /// that runs as a monochrome Game Boy can: a Game Boy Color cart never does.
    pub sgb: bool,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Option<Header> {
        let bytes = bytes.get(..HEADER_LEN)?;
        let title = &bytes[TITLE_OFF as usize..][..TITLE_LEN];
        let end = title.iter().position(|b| *b == 0).unwrap_or(TITLE_LEN);
        let code = &bytes[CODE_OFF..CODE_OFF + 4];
        let is_code = code
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        let cgb = bytes[CGB_OFF as usize];
        Some(Header {
            title: String::from_utf8_lossy(&title[..end]).trim().to_string(),
            code: if is_code {
                String::from_utf8_lossy(code).into_owned()
            } else {
                String::new()
            },
            cgb,
            japan: bytes[DEST_OFF] == 0,
            sgb: sgb_of(cgb, bytes[SGB_OFF as usize]),
        })
    }

    pub fn class(&self) -> Class {
        class_of(Some(self.cgb))
    }
}

pub fn header(rom: &Path) -> Option<Header> {
    let mut buf = [0u8; HEADER_LEN];
    read_at(rom, 0, &mut buf)?;
    Header::parse(&buf)
}

pub fn cached_header(rom: &Path) -> Option<Header> {
    crate::header_index::gb_header(rom)
}

pub fn cached_class(rom: &Path) -> Class {
    match cached_header(rom) {
        Some(h) => h.class(),
        None => class(rom),
    }
}

pub fn title(rom: &Path) -> Option<String> {
    let mut buf = [0u8; TITLE_LEN];
    read_at(rom, TITLE_OFF, &mut buf)?;
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    let text = std::str::from_utf8(&buf[..end]).ok()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Whether the cart supports Super Game Boy functions, read from its header cache.
pub fn is_sgb(rom: &Path) -> bool {
    cached_header(rom).is_some_and(|h| h.sgb)
}

fn sgb_of(cgb: u8, sgb: u8) -> bool {
    !matches!(cgb, 0x80 | 0xc0) && sgb == 0x03
}

pub fn cgb_flag(rom: &Path) -> Option<u8> {
    let mut buf = [0u8; 1];
    read_at(rom, CGB_OFF, &mut buf)?;
    Some(buf[0])
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Class {
    Original,
    DualMode,
    ColourOnly,
}

pub fn class(rom: &Path) -> Class {
    class_of(cgb_flag(rom))
}

fn class_of(flag: Option<u8>) -> Class {
    match flag {
        Some(0xc0) => Class::ColourOnly,
        Some(0x80) => Class::DualMode,
        _ => Class::Original,
    }
}

fn read_at(rom: &Path, off: u64, buf: &mut [u8]) -> Option<()> {
    let mut f = File::open(rom).ok()?;
    f.seek(SeekFrom::Start(off)).ok()?;
    f.read_exact(buf).ok()
}
