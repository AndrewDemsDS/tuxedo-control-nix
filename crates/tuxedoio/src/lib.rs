//! Minimal wrappers over the TUXEDO `/dev/tuxedo_io` Uniwill ioctl interface.
//!
//! Protocol + scaling derived from `tuxedo-drivers` and validated on an InfinityBook Pro
//! AMD Gen9 (Uniwill "NB02" path). See `docs/phase1-protocol.md`.

use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;

// ---- Linux _IOC encoding ----
const NRBITS: u32 = 8;
const TYPEBITS: u32 = 8;
const SIZEBITS: u32 = 14;
const TYPESHIFT: u32 = NRBITS;
const SIZESHIFT: u32 = TYPESHIFT + TYPEBITS;
const DIRSHIFT: u32 = SIZESHIFT + SIZEBITS;
const DIR_NONE: u32 = 0;
const DIR_WRITE: u32 = 1;
const DIR_READ: u32 = 2;

const fn ioc(dir: u32, ty: u32, nr: u32, size: u32) -> libc::c_ulong {
    ((dir << DIRSHIFT) | (ty << TYPESHIFT) | nr | (size << SIZESHIFT)) as libc::c_ulong
}
// IMPORTANT: the driver header types the arg as `int32_t*` / `char*` (a POINTER), so the
// encoded size is sizeof(pointer)=8 on x86_64, not sizeof(int32_t). (Empirically required.)
const PTR: u32 = 8;
const fn ior(ty: u32, nr: u32) -> libc::c_ulong {
    ioc(DIR_READ, ty, nr, PTR)
}
const fn iow(ty: u32, nr: u32) -> libc::c_ulong {
    ioc(DIR_WRITE, ty, nr, PTR)
}

const MAGIC: u32 = 0xEC;
const RD_UW: u32 = MAGIC + 3;
const WR_UW: u32 = MAGIC + 4;

pub const R_MOD_VERSION: libc::c_ulong = ior(MAGIC, 0x00);
pub const R_HWCHECK_UW: libc::c_ulong = ior(MAGIC, 0x06);
pub const R_UW_MODEL_ID: libc::c_ulong = ior(RD_UW, 0x01);
pub const R_UW_FANSPEED: libc::c_ulong = ior(RD_UW, 0x10);
pub const R_UW_FANSPEED2: libc::c_ulong = ior(RD_UW, 0x11);
pub const R_UW_FAN_TEMP: libc::c_ulong = ior(RD_UW, 0x12);
pub const R_UW_FAN_TEMP2: libc::c_ulong = ior(RD_UW, 0x13);
pub const R_UW_MODE: libc::c_ulong = ior(RD_UW, 0x14); // EC RAM 0x0751
pub const R_UW_FANS_OFF_AVAILABLE: libc::c_ulong = ior(RD_UW, 0x16);
pub const R_UW_FANS_MIN_SPEED: libc::c_ulong = ior(RD_UW, 0x17);
pub const R_UW_PROFS_AVAILABLE: libc::c_ulong = ior(RD_UW, 0x21);
pub const W_UW_FANSPEED: libc::c_ulong = iow(WR_UW, 0x10);
pub const W_UW_FANSPEED2: libc::c_ulong = iow(WR_UW, 0x11);
pub const W_UW_MODE: libc::c_ulong = iow(WR_UW, 0x12); // EC RAM 0x0751
pub const W_UW_FANAUTO: libc::c_ulong = ioc(DIR_NONE, WR_UW, 0x14, 0);
pub const W_UW_PERF_PROF: libc::c_ulong = iow(WR_UW, 0x18);

/// Fan duty 100% on the NB02 path = 0xc8 (200).
pub const FAN_MAX: i32 = 0xc8;
/// Bit in EC RAM 0x0751: when SET, `uw_set_fan` is a no-op and the EC owns the fan.
pub const FAN_OWNERSHIP_BIT: i32 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerfProfile {
    PowerSave = 1,
    Enthusiast = 2,
    Overboost = 3,
}

impl PerfProfile {
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "power_save" | "powersave" | "power-save" => Some(Self::PowerSave),
            "enthusiast" | "balanced" => Some(Self::Enthusiast),
            "overboost" | "performance" => Some(Self::Overboost),
            _ => None,
        }
    }
    /// Canonical id used in config + the GUI/socket protocol (stable, underscored).
    pub fn as_id(&self) -> &'static str {
        match self {
            Self::PowerSave => "power_save",
            Self::Enthusiast => "enthusiast",
            Self::Overboost => "overboost",
        }
    }
}

pub fn pct_to_raw(p: i32) -> i32 {
    (p.clamp(0, 100) * FAN_MAX / 100).clamp(0, FAN_MAX)
}
pub fn raw_to_pct(r: i32) -> i32 {
    (r * 100 + FAN_MAX / 2) / FAN_MAX
}

pub struct TuxedoIo(std::fs::File);

impl TuxedoIo {
    pub fn open() -> io::Result<Self> {
        Ok(Self(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tuxedo_io")?,
        ))
    }

    pub fn rd(&self, req: libc::c_ulong) -> io::Result<i32> {
        let mut v: i32 = 0;
        let r = unsafe { libc::ioctl(self.0.as_raw_fd(), req, &mut v as *mut i32) };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(v)
    }
    pub fn wr(&self, req: libc::c_ulong, mut v: i32) -> io::Result<()> {
        let r = unsafe { libc::ioctl(self.0.as_raw_fd(), req, &mut v as *mut i32) };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn noarg(&self, req: libc::c_ulong) -> io::Result<()> {
        let r = unsafe { libc::ioctl(self.0.as_raw_fd(), req) };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn is_uniwill(&self) -> io::Result<bool> {
        Ok(self.rd(R_HWCHECK_UW)? == 1)
    }
    pub fn cpu_temp(&self) -> io::Result<i32> {
        self.rd(R_UW_FAN_TEMP)
    }
    pub fn gpu_temp(&self) -> io::Result<i32> {
        self.rd(R_UW_FAN_TEMP2)
    }
    pub fn cpu_fan_pct(&self) -> io::Result<i32> {
        Ok(raw_to_pct(self.rd(R_UW_FANSPEED)?))
    }
    pub fn gpu_fan_pct(&self) -> io::Result<i32> {
        Ok(raw_to_pct(self.rd(R_UW_FANSPEED2)?))
    }
    pub fn mode(&self) -> io::Result<i32> {
        self.rd(R_UW_MODE)
    }

    /// Clear bit 0x40 of 0x0751 so manual fan writes are honoured. Returns true if it
    /// had to fix it (i.e. the EC had grabbed fan ownership).
    pub fn ensure_manual(&self) -> io::Result<bool> {
        let m = self.mode()?;
        if m & FAN_OWNERSHIP_BIT != 0 {
            self.wr(W_UW_MODE, m & !FAN_OWNERSHIP_BIT)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Set both fans to a duty percent, clearing the ownership bit first (the bug fix).
    pub fn set_fan_pct(&self, pct: i32) -> io::Result<()> {
        self.ensure_manual()?;
        let raw = pct_to_raw(pct);
        self.wr(W_UW_FANSPEED, raw)?;
        self.wr(W_UW_FANSPEED2, raw)?;
        Ok(())
    }

    pub fn set_perf(&self, p: PerfProfile) -> io::Result<()> {
        self.wr(W_UW_PERF_PROF, p as i32)
    }
    pub fn restore_auto(&self) -> io::Result<()> {
        self.noarg(W_UW_FANAUTO)
    }

    pub fn version(&self) -> io::Result<String> {
        let mut buf = [0u8; 64];
        let r = unsafe { libc::ioctl(self.0.as_raw_fd(), R_MOD_VERSION, buf.as_mut_ptr()) };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
    }
}
