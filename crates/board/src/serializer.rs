//! `.ref` board persistence: MessagePack wire format, bitmaps deduped by SHA-256.
//!
//! Hand-rolled MessagePack subset (no new deps): ints, int64, float32,
//! UTF-8 strings, byte arrays, arrays, bools. The reader also accepts the
//! compact family encodings (fixint/fixstr/fixarray, uints, float64) so files
//! written by the app's MessagePack packer decode here.

pub const SERIAL_VERSION: i32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct RefEntry {
    /// Real pixel bounds (left, top, right, bottom).
    pub bounds: (i32, i32, i32, i32),
    pub pos: (f32, f32),
    pub rotation: f32,
    pub scale: (f32, f32),
    /// Crop rect (left, top, right, bottom) in content px.
    pub crop: (f32, f32, f32, f32),
    pub flip_h: bool,
    pub flip_v: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefImage {
    pub sha: String,
    pub bitmap: Vec<u8>,
    pub instances: Vec<RefEntry>,
}

/// Text-sticker sidecar, keyed by position in [`RefBoardData::images`].
/// The `bitmap` slot of a text image entry is written empty; the shell
/// re-renders text at load.
#[derive(Clone, Debug, PartialEq)]
pub struct RefText {
    pub image_index: usize,
    pub text: String,
    pub size_px: f32,
    pub color: (u8, u8, u8),
    pub background: Option<(u8, u8, u8)>,
    pub bold: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefBoardData {
    pub version: i32,
    pub date: i64,
    /// Board canvas matrix values (9 floats).
    pub canvas: [f32; 9],
    pub images: Vec<RefImage>,
    pub texts: Vec<RefText>,
}

fn push_int(out: &mut Vec<u8>, v: i64) {
    if (-32..=127).contains(&v) {
        out.push(v as u8);
    } else if v >= i64::from(i8::MIN) && v <= i64::from(i8::MAX) {
        out.push(0xd0);
        out.push(v as u8);
    } else if v >= i64::from(i16::MIN) && v <= i64::from(i16::MAX) {
        out.push(0xd1);
        out.extend_from_slice(&(v as i16).to_be_bytes());
    } else if v >= i64::from(i32::MIN) && v <= i64::from(i32::MAX) {
        out.push(0xd2);
        out.extend_from_slice(&(v as i32).to_be_bytes());
    } else {
        out.push(0xd3);
        out.extend_from_slice(&v.to_be_bytes());
    }
}

fn push_uint(out: &mut Vec<u8>, v: u64) {
    if v <= 127 {
        out.push(v as u8);
    } else if v <= u64::from(u8::MAX) {
        out.push(0xcc);
        out.push(v as u8);
    } else if v <= u64::from(u16::MAX) {
        out.push(0xcd);
        out.extend_from_slice(&(v as u16).to_be_bytes());
    } else if v <= u64::from(u32::MAX) {
        out.push(0xce);
        out.extend_from_slice(&(v as u32).to_be_bytes());
    } else {
        out.push(0xcf);
        out.extend_from_slice(&v.to_be_bytes());
    }
}

fn push_f32(out: &mut Vec<u8>, v: f32) {
    out.push(0xca);
    out.extend_from_slice(&v.to_be_bytes());
}

fn push_bool(out: &mut Vec<u8>, v: bool) {
    out.push(if v { 0xc3 } else { 0xc2 });
}

fn push_array(out: &mut Vec<u8>, n: usize) {
    if n <= usize::from(u16::MAX) {
        out.push(0xdc);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push(0xdd);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    }
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    if b.len() <= u8::MAX as usize {
        out.push(0xd9);
        out.push(b.len() as u8);
    } else if b.len() <= usize::from(u16::MAX) {
        out.push(0xda);
        out.extend_from_slice(&(b.len() as u16).to_be_bytes());
    } else {
        out.push(0xdb);
        out.extend_from_slice(&(b.len() as u32).to_be_bytes());
    }
    out.extend_from_slice(b);
}

fn push_bin(out: &mut Vec<u8>, b: &[u8]) {
    if b.len() <= u8::MAX as usize {
        out.push(0xc4);
        out.push(b.len() as u8);
    } else if b.len() <= usize::from(u16::MAX) {
        out.push(0xc5);
        out.extend_from_slice(&(b.len() as u16).to_be_bytes());
    } else {
        out.push(0xc6);
        out.extend_from_slice(&(b.len() as u32).to_be_bytes());
    }
    out.extend_from_slice(b);
}

pub fn encode(board: &RefBoardData) -> Vec<u8> {
    let mut out = Vec::new();
    push_int(&mut out, i64::from(board.version));
    push_int(&mut out, board.date);
    for v in board.canvas {
        push_f32(&mut out, v);
    }
    push_array(&mut out, board.images.len());
    for img in &board.images {
        push_str(&mut out, &img.sha);
        push_bin(&mut out, &img.bitmap);
        push_array(&mut out, img.instances.len());
        for e in &img.instances {
            push_int(&mut out, i64::from(e.bounds.0));
            push_int(&mut out, i64::from(e.bounds.1));
            push_int(&mut out, i64::from(e.bounds.2));
            push_int(&mut out, i64::from(e.bounds.3));
            push_f32(&mut out, e.pos.0);
            push_f32(&mut out, e.pos.1);
            push_f32(&mut out, e.rotation);
            push_f32(&mut out, e.scale.0);
            push_f32(&mut out, e.scale.1);
            push_f32(&mut out, e.crop.0);
            push_f32(&mut out, e.crop.1);
            push_f32(&mut out, e.crop.2);
            push_f32(&mut out, e.crop.3);
            push_bool(&mut out, e.flip_h);
            push_bool(&mut out, e.flip_v);
        }
    }
    push_array(&mut out, board.texts.len());
    for t in &board.texts {
        push_uint(&mut out, t.image_index as u64);
        push_str(&mut out, &t.text);
        push_f32(&mut out, t.size_px);
        for v in [t.color.0, t.color.1, t.color.2] {
            push_uint(&mut out, u64::from(v));
        }
        push_uint(&mut out, u64::from(t.background.is_some() as u8));
        let bg = t.background.unwrap_or((0, 0, 0));
        for v in [bg.0, bg.1, bg.2] {
            push_uint(&mut out, u64::from(v));
        }
        push_uint(&mut out, u64::from(t.bold as u8));
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8], String> {
        let end = self.pos.saturating_add(n);
        if end > self.b.len() {
            return Err(format!("ref: truncated {what}"));
        }
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1, "value")?[0])
    }

    fn be(&mut self, n: usize, what: &str) -> Result<&'a [u8], String> {
        self.take(n, what)
    }

    fn int(&mut self) -> Result<i64, String> {
        let tag = self.byte()?;
        match tag {
            0x00..=0x7f => Ok(i64::from(tag)),
            0xe0..=0xff => Ok(i64::from(tag as i8)),
            0xcc => Ok(i64::from(self.byte()?)),
            0xcd => Ok(i64::from(u16::from_be_bytes(
                self.be(2, "uint16")?.try_into().unwrap(),
            ))),
            0xce => Ok(i64::from(u32::from_be_bytes(
                self.be(4, "uint32")?.try_into().unwrap(),
            ))),
            0xcf => i64::try_from(u64::from_be_bytes(
                self.be(8, "uint64")?.try_into().unwrap(),
            ))
            .map_err(|_| "ref: int overflow".to_string()),
            0xd0 => Ok(i64::from(self.byte()? as i8)),
            0xd1 => Ok(i64::from(i16::from_be_bytes(
                self.be(2, "int16")?.try_into().unwrap(),
            ))),
            0xd2 => Ok(i64::from(i32::from_be_bytes(
                self.be(4, "int32")?.try_into().unwrap(),
            ))),
            0xd3 => Ok(i64::from_be_bytes(
                self.be(8, "int64")?.try_into().unwrap(),
            )),
            _ => Err("ref: expected int".to_string()),
        }
    }

    fn int32(&mut self) -> Result<i32, String> {
        i32::try_from(self.int()?).map_err(|_| "ref: int out of range".to_string())
    }

    fn small(&mut self, what: &str) -> Result<u8, String> {
        u8::try_from(self.int()?).map_err(|_| format!("ref: {what} out of range"))
    }

    fn float(&mut self) -> Result<f32, String> {
        match self.byte()? {
            0xca => Ok(f32::from_be_bytes(
                self.be(4, "float32")?.try_into().unwrap(),
            )),
            0xcb => Ok(f64::from_be_bytes(
                self.be(8, "float64")?.try_into().unwrap(),
            ) as f32),
            _ => Err("ref: expected float".to_string()),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        let tag = self.byte()?;
        let len = match tag {
            0xa0..=0xbf => usize::from(tag & 0x1f),
            0xd9 => usize::from(self.byte()?),
            0xda => usize::from(u16::from_be_bytes(
                self.be(2, "str16")?.try_into().unwrap(),
            )),
            0xdb => usize::try_from(u32::from_be_bytes(
                self.be(4, "str32")?.try_into().unwrap(),
            ))
            .map_err(|_| "ref: str too long".to_string())?,
            _ => return Err("ref: expected str".to_string()),
        };
        let b = self.take(len, "str bytes")?;
        std::str::from_utf8(b)
            .map(str::to_string)
            .map_err(|_| "ref: invalid utf-8".to_string())
    }

    fn bin(&mut self) -> Result<Vec<u8>, String> {
        let len = match self.byte()? {
            0xc4 => usize::from(self.byte()?),
            0xc5 => usize::from(u16::from_be_bytes(
                self.be(2, "bin16")?.try_into().unwrap(),
            )),
            0xc6 => usize::try_from(u32::from_be_bytes(
                self.be(4, "bin32")?.try_into().unwrap(),
            ))
            .map_err(|_| "ref: bin too long".to_string())?,
            _ => return Err("ref: expected bin".to_string()),
        };
        Ok(self.take(len, "bin bytes")?.to_vec())
    }

    fn array(&mut self) -> Result<usize, String> {
        let tag = self.byte()?;
        match tag {
            0x90..=0x9f => Ok(usize::from(tag & 0x0f)),
            0xdc => Ok(usize::from(u16::from_be_bytes(
                self.be(2, "array16")?.try_into().unwrap(),
            ))),
            0xdd => Ok(usize::try_from(u32::from_be_bytes(
                self.be(4, "array32")?.try_into().unwrap(),
            ))
            .map_err(|_| "ref: array too long".to_string())?),
            _ => Err("ref: expected array".to_string()),
        }
    }

    fn bool(&mut self) -> Result<bool, String> {
        match self.byte()? {
            0xc2 => Ok(false),
            0xc3 => Ok(true),
            _ => Err("ref: expected bool".to_string()),
        }
    }

    fn entry(&mut self) -> Result<RefEntry, String> {
        Ok(RefEntry {
            bounds: (self.int32()?, self.int32()?, self.int32()?, self.int32()?),
            pos: (self.float()?, self.float()?),
            rotation: self.float()?,
            scale: (self.float()?, self.float()?),
            crop: (self.float()?, self.float()?, self.float()?, self.float()?),
            flip_h: self.bool()?,
            flip_v: self.bool()?,
        })
    }
}

pub fn decode(bytes: &[u8]) -> Result<RefBoardData, String> {
    let mut r = Reader { b: bytes, pos: 0 };
    let version = r.int32()?;
    let date = r.int()?;
    let mut canvas = [0.0f32; 9];
    for v in &mut canvas {
        *v = r.float()?;
    }
    let mut images = Vec::new();
    for _ in 0..r.array()? {
        let sha = r.string()?;
        let bitmap = r.bin()?;
        let mut instances = Vec::new();
        for _ in 0..r.array()? {
            instances.push(r.entry()?);
        }
        images.push(RefImage {
            sha,
            bitmap,
            instances,
        });
    }
    let mut texts = Vec::new();
    for _ in 0..r.array()? {
        let index = r.int()?;
        if !(0..=65535).contains(&index) {
            return Err("ref: image index out of range".to_string());
        }
        let text = r.string()?;
        let size_px = r.float()?;
        let color = (r.small("color")?, r.small("color")?, r.small("color")?);
        let has_bg = match r.small("has_bg")? {
            0 => false,
            1 => true,
            _ => return Err("ref: bad has_bg flag".to_string()),
        };
        let bg = (r.small("bg")?, r.small("bg")?, r.small("bg")?);
        let bold = match r.small("bold")? {
            0 => false,
            1 => true,
            _ => return Err("ref: bad bold flag".to_string()),
        };
        texts.push(RefText {
            image_index: index as usize,
            text,
            size_px,
            color,
            background: has_bg.then_some(bg),
            bold,
        });
    }
    if r.pos != bytes.len() {
        return Err("ref: trailing bytes".to_string());
    }
    Ok(RefBoardData {
        version,
        date,
        canvas,
        images,
        texts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(l: i32, flip_h: bool) -> RefEntry {
        RefEntry {
            bounds: (l, -5, 300, 400),
            pos: (12.5, -3.25),
            rotation: 45.0,
            scale: (1.5, 2.0),
            crop: (0.0, 0.0, 300.0, 400.0),
            flip_h,
            flip_v: false,
        }
    }

    fn board() -> RefBoardData {
        let bitmap = vec![0x89u8, 0x50, 0x4e, 0x47, 0x00, 0xff];
        RefBoardData {
            version: SERIAL_VERSION,
            date: 1_728_364_800_000,
            canvas: [1.0, 0.0, 10.0, 0.0, 1.0, 20.0, 0.0, 0.0, 1.0],
            images: vec![
                RefImage {
                    sha: "aa".repeat(32),
                    bitmap: bitmap.clone(),
                    instances: vec![entry(0, false)],
                },
                RefImage {
                    sha: "aa".repeat(32),
                    bitmap,
                    instances: vec![entry(50, true)],
                },
            ],
            texts: vec![],
        }
    }

    #[test]
    fn roundtrip_two_images_shared_sha() {
        let b = board();
        assert_eq!(decode(&encode(&b)), Ok(b));
    }

    #[test]
    fn roundtrip_empty() {
        let b = RefBoardData {
            version: SERIAL_VERSION,
            date: 0,
            canvas: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            images: vec![],
            texts: vec![],
        };
        assert_eq!(decode(&encode(&b)), Ok(b));
    }

    #[test]
    fn roundtrip_text_sidecar() {
        let mut b = board();
        b.texts = vec![
            RefText {
                image_index: 0,
                text: "hello".to_string(),
                size_px: 24.0,
                color: (200, 10, 5),
                background: None,
                bold: true,
            },
            RefText {
                image_index: 1,
                text: "".to_string(),
                size_px: 12.5,
                color: (0, 0, 0),
                background: Some((255, 255, 255)),
                bold: false,
            },
        ];
        assert_eq!(decode(&encode(&b)), Ok(b));
    }

    #[test]
    fn rejects_truncated() {
        let enc = encode(&board());
        for cut in [0, 1, 5, enc.len() / 2, enc.len() - 1] {
            assert!(decode(&enc[..cut]).is_err(), "cut at {cut} decoded");
        }
        assert!(decode(&[]).is_err());
        assert!(decode(&enc).is_ok());
    }

    #[test]
    fn rejects_trailing_and_garbage() {
        let mut enc = encode(&board());
        enc.push(0x00);
        assert!(decode(&enc).is_err());
        assert!(decode(&[0x01, 0x02, 0x03]).is_err());
    }

    #[test]
    fn reads_compact_family() {
        // App-packer style: fixint, fixstr, fixarray, uint8.
        let mut b: Vec<u8> = vec![0x01, 0xd3];
        b.extend_from_slice(&1_728_364_800_000i64.to_be_bytes());
        for v in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
            b.push(0xca);
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.push(0x91); // one image
        b.push(0xa2);
        b.extend_from_slice(b"ab"); // fixstr sha
        b.extend_from_slice(&[0xc4, 0x02, 0x07, 0x08]); // bin
        b.push(0x91); // one entry
        b.extend_from_slice(&[0x00, 0x00, 0x0a, 0x14]); // bounds 0,0,10,20
        for v in [1.5f32, -2.5, 0.0, 1.0, 1.0, 0.0, 0.0, 10.0, 20.0] {
            b.push(0xca);
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&[0xc2, 0xc3]); // flip_h false, flip_v true
        b.push(0x91); // one text
        b.push(0x00); // image index
        b.push(0xa2);
        b.extend_from_slice(b"hi");
        b.push(0xca);
        b.extend_from_slice(&12.5f32.to_be_bytes());
        b.extend_from_slice(&[0xcc, 200, 0x0a, 0x05]); // color
        b.extend_from_slice(&[0x01, 0x01, 0x02, 0x03, 0x00]); // bg + bold
        let got = decode(&b).expect("compact board decodes");
        assert_eq!(got.version, 1);
        assert_eq!(got.date, 1_728_364_800_000);
        assert_eq!(got.images.len(), 1);
        assert_eq!(got.images[0].sha, "ab");
        assert_eq!(got.images[0].bitmap, vec![7, 8]);
        assert_eq!(got.images[0].instances[0].bounds, (0, 0, 10, 20));
        assert!(got.images[0].instances[0].flip_v);
        assert_eq!(got.texts.len(), 1);
        assert_eq!(got.texts[0].color, (200, 10, 5));
        assert_eq!(got.texts[0].background, Some((1, 2, 3)));
    }
}
