use crate::protocol::{self, Config, Frame, FRAME_LEN};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const VID: u32 = 0x3554;
const PID: u32 = 0xFA09;
pub const NO_ANSWER: &str = "Keyboard not answering. Charge it or press a key, then Reload.";

pub fn find() -> Result<PathBuf, String> {
    let mut hits = Vec::new();
    let entries = fs::read_dir("/sys/class/hidraw").map_err(|e| e.to_string())?;
    for e in entries.flatten() {
        let dev = e.path().join("device");
        let uevent = fs::read_to_string(dev.join("uevent")).unwrap_or_default();
        let Some(id) = uevent.lines().find_map(|l| l.strip_prefix("HID_ID=")) else { continue };
        let parts: Vec<u32> = id.split(':').filter_map(|p| u32::from_str_radix(p, 16).ok()).collect();
        if parts.len() != 3 || parts[1] != VID || parts[2] != PID {
            continue;
        }
        let desc = fs::read(dev.join("report_descriptor")).unwrap_or_default();
        let has = |pat: &[u8]| desc.windows(pat.len()).any(|w| w == pat);
        if has(&[0x85, 0x13]) && has(&[0x95, 0x13]) && has(&[0x91]) {
            hits.push(PathBuf::from("/dev").join(e.file_name()));
        }
    }
    match hits.len() {
        0 => Err("receiver 3554:fa09 not found".into()),
        1 => Ok(hits.remove(0)),
        _ => Err(format!("more than one receiver found: {hits:?}")),
    }
}

pub struct Device {
    file: File,
    pub path: PathBuf,
}

impl Device {
    pub fn open() -> Result<Self, String> {
        let path = find()?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Device { file, path })
    }

    fn send(&mut self, f: &Frame) -> Result<(), String> {
        let n = self.file.write(f).map_err(|e| e.to_string())?;
        if n != FRAME_LEN {
            return Err(format!("short write {n}/{FRAME_LEN}"));
        }
        Ok(())
    }

    fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>, String> {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        let mut pfd = libc::pollfd { fd: self.file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let r = unsafe { libc::poll(&mut pfd, 1, left.as_millis().max(1) as i32) };
        if r < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if r == 0 {
            return Ok(None);
        }
        let mut buf = [0u8; 64];
        match self.file.read(&mut buf) {
            Ok(n) => Ok(Some(buf[..n].to_vec())),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Some(Vec::new())),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn read_config(&mut self) -> Result<Config, String> {
        let mut got: Vec<Frame> = Vec::new();
        for _ in 0..3 {
            self.send(&protocol::read_request())?;
            let deadline = Instant::now() + Duration::from_millis(600);
            while let Some(buf) = self.recv(deadline)? {
                if protocol::is_read_response(&buf) && protocol::valid(&buf) {
                    let f: Frame = buf.try_into().unwrap();
                    got.retain(|g| g[3] != f[3]);
                    got.push(f);
                    if got.len() == protocol::FRAGMENTS {
                        return Config::from_frames(&got);
                    }
                }
            }
        }
        if got.is_empty() {
            return Err(NO_ANSWER.into());
        }
        Config::from_frames(&got)
    }

    fn send_acked(&mut self, f: &Frame) -> Result<(), String> {
        self.send(f)?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while let Some(buf) = self.recv(deadline)? {
            if buf == f {
                return Ok(());
            }
        }
        Err(format!("no echo for {}", protocol::hex(f)))
    }

    pub fn write(&mut self, frames: &[Frame]) -> Result<(), String> {
        for f in frames {
            self.send_acked(f)?;
        }
        self.send_acked(&protocol::save_request())
    }
}

impl Device {
    pub fn apply(&mut self, want: protocol::Lighting, color: Option<[u8; 3]>) -> Result<Config, String> {
        let before = self.read_config()?;
        let mut frames = before.write_frames(Some(want));
        if let Some(rgb) = color {
            frames.extend(protocol::palette_frames(rgb, want.effect != 1));
        }
        self.write(&frames)?;
        let after = self.read_config()?;
        let got = after.lighting();
        if got != want {
            return Err(format!("read-back mismatch: wanted {want:?}, keyboard has {got:?}"));
        }
        Ok(after)
    }
}
