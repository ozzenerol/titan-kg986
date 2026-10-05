pub const REPORT_ID: u8 = 0x13;
pub const FRAME_LEN: usize = 20;
pub const FRAGMENTS: usize = 10;

const CMD_READ: u8 = 0x44;
const CMD_WRITE: u8 = 0x04;
const CMD_COLOR: u8 = 0x09;
const CMD_SAVE: u8 = 0x0A;
const SUB_READ_REQ: u8 = 0x01;
const SUB_READ_RESP: u8 = 0x0A;
const SUB_PALETTE: u8 = 0x25;

pub type Frame = [u8; FRAME_LEN];

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

fn seal(mut f: Frame) -> Frame {
    f[FRAME_LEN - 1] = checksum(&f[..FRAME_LEN - 1]);
    f
}

pub fn valid(f: &[u8]) -> bool {
    f.len() == FRAME_LEN && checksum(&f[..FRAME_LEN - 1]) == f[FRAME_LEN - 1]
}

pub fn read_request() -> Frame {
    let mut f = [0u8; FRAME_LEN];
    f[..4].copy_from_slice(&[REPORT_ID, CMD_READ, SUB_READ_REQ, 0]);
    seal(f)
}

pub fn save_request() -> Frame {
    let mut f = [0u8; FRAME_LEN];
    f[..6].copy_from_slice(&[REPORT_ID, CMD_SAVE, 0x01, 0, 4, 7]);
    seal(f)
}

pub fn is_read_response(f: &[u8]) -> bool {
    f.len() == FRAME_LEN && f[0] == REPORT_ID && f[1] == CMD_READ && f[2] == SUB_READ_RESP
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub frames: [Frame; FRAGMENTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lighting {
    pub effect: u8,
    pub brightness: u8,
    pub speed: u8,
    pub colorful: bool,
}

fn slot(effect: u8) -> Option<(usize, usize)> {
    match effect {
        1..=6 => Some((4, 7 + (effect as usize - 1) * 2)),
        7..=13 => Some((5, 5 + (effect as usize - 7) * 2)),
        14..=18 => Some((6, 5 + (effect as usize - 14) * 2)),
        _ => None,
    }
}

impl Config {
    pub fn from_frames(frames: &[Frame]) -> Result<Self, String> {
        let mut out: [Option<Frame>; FRAGMENTS] = [None; FRAGMENTS];
        for f in frames {
            if !valid(f) || !is_read_response(f) {
                return Err(format!("bad frame {}", hex(f)));
            }
            let seq = f[3] as usize;
            if seq >= FRAGMENTS {
                return Err(format!("bad sequence {seq}"));
            }
            out[seq] = Some(*f);
        }
        let missing: Vec<usize> = (0..FRAGMENTS).filter(|i| out[*i].is_none()).collect();
        if !missing.is_empty() {
            return Err(format!("missing fragments {missing:?}"));
        }
        Ok(Config { frames: out.map(|f| f.unwrap()) })
    }

    pub fn effect(&self) -> u8 {
        self.frames[0][15]
    }

    pub fn lighting(&self) -> Lighting {
        let effect = self.effect();
        match slot(effect) {
            Some((frag, off)) => {
                let f = &self.frames[frag];
                Lighting {
                    effect,
                    brightness: f[off],
                    speed: f[off + 1] >> 4,
                    colorful: f[off + 1] & 0x0F == 0x07,
                }
            }
            None => Lighting { effect, brightness: 0, speed: 0, colorful: false },
        }
    }

    pub fn write_frames(&self, l: Option<Lighting>) -> Vec<Frame> {
        let mut frames = self.frames;
        for f in frames.iter_mut() {
            f[1] = CMD_WRITE;
        }
        frames[0][8] = 0x01;
        frames[0][14] = 0x00;
        frames[0][17] = 0x01;
        if let Some(l) = l {
            frames[0][15] = l.effect;
            if let Some((frag, off)) = slot(l.effect) {
                frames[frag][off] = l.brightness;
                frames[frag][off + 1] = (l.speed << 4) | if l.colorful { 0x07 } else { 0x00 };
            }
        }
        frames.into_iter().map(seal).collect()
    }

    pub fn to_hex(&self) -> String {
        self.frames.iter().map(|f| hex(f) + "\n").collect()
    }

    pub fn from_hex(s: &str) -> Result<Self, String> {
        let frames = s
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(parse_hex)
            .collect::<Result<Vec<_>, _>>()?;
        Config::from_frames(&frames)
    }
}

fn color_frame(seq: u8, payload: [u8; 15]) -> Frame {
    let mut f = [0u8; FRAME_LEN];
    f[..4].copy_from_slice(&[REPORT_ID, CMD_COLOR, SUB_PALETTE, seq]);
    f[4..19].copy_from_slice(&payload);
    seal(f)
}

pub fn palette_frames(rgb: [u8; 3], monochrome: bool) -> Vec<Frame> {
    let mut payloads: Vec<[u8; 15]> = Vec::with_capacity(37);
    if monochrome {
        let stream: Vec<u8> = rgb.iter().copied().cycle().take(36 * 14).collect();
        for chunk in stream.chunks(14) {
            let mut p = [0u8; 15];
            p[0] = 0x0E;
            p[1..].copy_from_slice(chunk);
            payloads.push(p);
        }
    } else {
        let mut zero = [0u8; 15];
        zero[0] = 0x0E;
        let mut second = zero;
        second[8..11].copy_from_slice(&rgb);
        second[12] = 0xFF;
        let patterns = [
            parse15("0e00ffffff00ff00ff00ffffffffff"),
            parse15("0eff000000ff000000ffffff00ff00"),
            parse15("0eff00ffffffffffff000000ff0000"),
        ];
        payloads.push(zero);
        payloads.push(second);
        payloads.extend((0..19).map(|i| patterns[i % 3]));
        payloads.extend(std::iter::repeat_n(zero, 15));
    }
    payloads.push(parse15("0800005aa500000000000000000000"));
    payloads
        .into_iter()
        .enumerate()
        .map(|(i, p)| color_frame(i as u8, p))
        .collect()
}

fn parse15(s: &str) -> [u8; 15] {
    let v = parse_bytes(s).expect("static hex");
    v.try_into().expect("15 bytes")
}

fn parse_bytes(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        return Err("odd hex length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

pub fn parse_hex(s: &str) -> Result<Frame, String> {
    parse_bytes(s)?
        .try_into()
        .map_err(|v: Vec<u8>| format!("expected {FRAME_LEN} bytes, got {}", v.len()))
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURED: &str = "\
13 44 0a 00 0e 00 03 03 02 00 00 04 04 07 00 06 20 01 00 ad
13 44 0a 01 0e 01 00 00 00 01 00 04 02 00 ff 04 00 00 00 7b
13 44 0a 02 0e 01 00 03 01 00 00 00 00 00 00 00 00 00 00 76
13 44 0a 03 0e 00 00 00 00 00 00 00 00 00 00 00 00 00 00 72
13 44 0a 04 0e ff ff 00 34 04 30 04 37 04 37 04 37 01 47 d2
13 44 0a 05 0e 04 37 04 37 04 37 04 37 04 37 00 37 04 31 07
13 44 0a 06 0e 04 37 04 37 04 37 04 37 04 37 04 37 04 37 12
13 44 0a 07 0e 07 47 07 47 07 44 07 44 07 44 07 44 07 44 89
13 44 0a 08 0e 07 44 07 44 04 04 04 04 04 04 04 04 04 04 35
13 44 0a 09 02 5a a5 00 00 00 00 00 00 00 00 00 00 00 00 6b
";

    #[test]
    fn read_request_matches_capture() {
        assert_eq!(hex(&read_request()), "13 44 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 58");
    }

    #[test]
    fn parses_captured_config() {
        let c = Config::from_hex(CAPTURED).unwrap();
        assert_eq!(
            c.lighting(),
            Lighting { effect: 6, brightness: 1, speed: 4, colorful: true }
        );
        assert_eq!(Config::from_hex(&c.to_hex()).unwrap(), c);
    }

    #[test]
    fn write_frames_are_sealed_and_targeted() {
        let c = Config::from_hex(CAPTURED).unwrap();
        let l = Lighting { effect: 2, brightness: 3, speed: 1, colorful: false };
        let w = c.write_frames(Some(l));
        assert_eq!(w.len(), FRAGMENTS);
        assert!(w.iter().all(|f| valid(f) && f[1] == CMD_WRITE));
        assert_eq!(w[0][15], 2);
        assert_eq!(&w[4][9..11], &[3, 0x10]);
    }

    #[test]
    fn palette_has_37_frames() {
        for mono in [false, true] {
            let p = palette_frames([1, 2, 3], mono);
            assert_eq!(p.len(), 37);
            assert!(p.iter().all(|f| valid(f)));
            assert_eq!(&p[36][4..8], &[0x08, 0, 0, 0x5a]);
        }
    }
}
