//! CPU and memory history for the small system monitor panel.
//!
//! The idle daemon / recorder runs all the time and writes a sample every INTERVAL seconds into a small
//! JSON file (same file and format as the Python version). So when the screensaver appears, the graph
//! already shows the last 30 minutes. Without a fresh file the screensaver samples by itself.
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use serde_json::Value;

use crate::util::now;

pub const INTERVAL: u64 = 5; // seconds between samples
pub const HISTORY: usize = (30 * 60 / INTERVAL) as usize; // 30 minutes

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SysData {
    pub interval: f64,
    pub time: f64,
    pub cpu: Vec<f64>,
    pub mem: Vec<f64>,
    pub mem_total: f64,
}

pub fn path() -> PathBuf {
    #[cfg(windows)]
    let dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(crate::sessions::home_dir)
        .join("ClaudeVivarium");
    #[cfg(target_os = "macos")]
    let dir = crate::sessions::home_dir().join("Library/Caches/ClaudeVivarium");
    #[cfg(not(any(windows, target_os = "macos")))]
    let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| "/tmp".into());
    dir.join("claude-vivarium-stats.json")
}

// ---------------------------------------------------------------- per platform: (total, idle) ticks, (used, total) bytes
#[cfg(target_os = "linux")]
mod os {
    pub fn read_cpu() -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/stat").ok()?;
        let vals: Vec<u64> = text.lines().next()?.split_whitespace().skip(1).take(8).filter_map(|v| v.parse().ok()).collect();
        if vals.len() < 5 {
            return None;
        }
        Some((vals.iter().sum(), vals[3] + vals[4])) // idle + iowait
    }

    pub fn read_mem() -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let get = |key: &str| {
            text.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        };
        let total = get("MemTotal:")?;
        Some((total.saturating_sub(get("MemAvailable:").unwrap_or(0)), total))
    }
}

#[cfg(target_os = "macos")]
mod os {
    use std::process::Command;

    extern "C" {
        fn mach_host_self() -> u32;
        fn host_statistics(host: u32, flavor: i32, info: *mut u32, count: *mut u32) -> i32;
        fn sysctlbyname(name: *const u8, old: *mut u8, oldlen: *mut usize, new: *const u8, newlen: usize) -> i32;
    }

    pub fn read_cpu() -> Option<(u64, u64)> {
        let mut ticks = [0u32; 4]; // HOST_CPU_LOAD_INFO: user, system, idle, nice
        let mut count = 4u32;
        // SAFETY: the buffer holds the 4 counters this flavour returns
        let rc = unsafe { host_statistics(mach_host_self(), 3, ticks.as_mut_ptr(), &mut count) };
        if rc != 0 {
            return None;
        }
        Some((ticks.iter().map(|t| *t as u64).sum(), ticks[2] as u64))
    }

    fn memsize() -> u64 {
        let mut val = 0u64;
        let mut size = 8usize;
        // SAFETY: hw.memsize is a 64-bit integer
        unsafe { sysctlbyname(c"hw.memsize".as_ptr().cast(), &mut val as *mut u64 as *mut u8, &mut size, std::ptr::null(), 0) };
        val
    }

    /// "Memory used" like Activity Monitor: app (active) + wired + compressed pages.
    pub fn read_mem() -> Option<(u64, u64)> {
        let out = Command::new("vm_stat").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let page = text
            .split("page size of ")
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(4096);
        let pages = |name: &str| {
            text.lines()
                .find(|l| l.starts_with(name))
                .and_then(|l| l.rsplit(':').next())
                .and_then(|v| v.trim().trim_end_matches('.').parse::<u64>().ok())
                .unwrap_or(0)
        };
        let used = (pages("Pages active") + pages("Pages wired down") + pages("Pages occupied by compressor")) * page;
        Some((used, memsize()))
    }
}

#[cfg(windows)]
mod os {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct MemStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_ext: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GlobalMemoryStatusEx(st: *mut MemStatus) -> i32;
    }

    pub fn read_cpu() -> Option<(u64, u64)> {
        let (mut idle, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default());
        // SAFETY: three valid out pointers
        if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return None;
        }
        let ft = |f: &FileTime| ((f.high as u64) << 32) | f.low as u64;
        Some((ft(&kernel) + ft(&user), ft(&idle))) // kernel time already includes idle time
    }

    pub fn read_mem() -> Option<(u64, u64)> {
        // SAFETY: the struct is plain data and carries its own size
        let mut st: MemStatus = unsafe { std::mem::zeroed() };
        st.length = std::mem::size_of::<MemStatus>() as u32;
        if unsafe { GlobalMemoryStatusEx(&mut st) } == 0 {
            return None;
        }
        Some((st.total_phys - st.avail_phys, st.total_phys))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod os {
    pub fn read_cpu() -> Option<(u64, u64)> {
        None
    }
    pub fn read_mem() -> Option<(u64, u64)> {
        None
    }
}

#[derive(Default)]
pub struct Sampler {
    prev: Option<(u64, u64)>,
    pub cpu: Vec<f64>,
    pub mem: Vec<f64>,
    pub mem_total: f64,
    pub last: f64,
}

impl Sampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// A sampler that continues the history of a file written up to a minute ago (quick restart).
    pub fn resumed() -> Self {
        let mut s = Self::new();
        if let Some(old) = load(60.0) {
            s.cpu = old.cpu;
            s.mem = old.mem;
        }
        s
    }

    /// Take one sample (CPU % since the previous call, memory used in bytes).
    pub fn sample(&mut self) {
        let (Some((total, idle)), Some((used, mtotal))) = (os::read_cpu(), os::read_mem()) else { return };
        if let Some((pt, pi)) = self.prev {
            let (dt, di) = (total.wrapping_sub(pt) as f64, idle.wrapping_sub(pi) as f64);
            self.cpu.push(if dt > 0.0 { ((100.0 * (1.0 - di / dt)) * 10.0).round() / 10.0 } else { 0.0 });
            self.mem.push(used as f64);
            for v in [&mut self.cpu, &mut self.mem] {
                if v.len() > HISTORY {
                    v.drain(..v.len() - HISTORY);
                }
            }
        }
        self.prev = Some((total, idle));
        self.mem_total = mtotal as f64;
        self.last = now();
    }

    pub fn data(&self) -> SysData {
        SysData {
            interval: INTERVAL as f64,
            time: self.last,
            cpu: self.cpu.clone(),
            mem: self.mem.clone(),
            mem_total: self.mem_total,
        }
    }

    pub fn save(&self) {
        let p = path();
        let tmp = p.with_extension("json.tmp");
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        let json = serde_json::json!({
            "interval": INTERVAL, "time": self.last, "cpu": self.cpu, "mem": self.mem,
            "mem_total": self.mem_total, "cores": cores,
        });
        if fs::write(&tmp, json.to_string()).is_ok() {
            let _ = fs::rename(&tmp, &p);
        }
    }
}

/// History written by the daemon, or None if missing or older than `max_age` seconds.
pub fn load(max_age: f64) -> Option<SysData> {
    let d: Value = serde_json::from_str(&fs::read_to_string(path()).ok()?).ok()?;
    let time = d.get("time").and_then(Value::as_f64).unwrap_or(0.0);
    if now() - time > max_age {
        return None;
    }
    let list = |k: &str| -> Vec<f64> {
        d.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default()
    };
    Some(SysData {
        interval: d.get("interval").and_then(Value::as_f64).unwrap_or(INTERVAL as f64),
        time,
        cpu: list("cpu"),
        mem: list("mem"),
        mem_total: d.get("mem_total").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

/// Used by the screensaver: prefers the daemon's history, falls back to sampling itself.
pub struct Monitor {
    own: Sampler,
    next_own: f64,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        let mut own = Sampler::new();
        own.sample();
        Monitor { own, next_own: now() + 1.0 }
    }

    pub fn get(&mut self) -> Option<Rc<SysData>> {
        if let Some(d) = load(3.0 * INTERVAL as f64) {
            if !d.cpu.is_empty() {
                return Some(Rc::new(d));
            }
        }
        let t = now();
        if t >= self.next_own {
            self.own.sample();
            self.next_own = t + INTERVAL as f64;
        }
        if self.own.cpu.is_empty() { None } else { Some(Rc::new(self.own.data())) }
    }
}

/// 30 minutes of made-up CPU/memory history for previews and benchmarks (same curve as the Python preview).
pub fn fake() -> SysData {
    let gib = 1024f64.powi(3);
    let cpu = (0..360)
        .map(|i| {
            let i = i as f64;
            (18.0 + 14.0 * (i / 23.0).sin() + 30.0 * (-((i - 250.0) / 18.0).powi(2)).exp() + 6.0 * (i * 1.7).sin())
                .clamp(2.0, 100.0)
        })
        .collect();
    let mem = (0..360).map(|i| (6.2 + 1.5 * (i as f64 / 70.0).sin() + 0.002 * i as f64) * gib).collect();
    SysData { interval: INTERVAL as f64, time: now(), cpu, mem, mem_total: 15.4 * gib }
}
