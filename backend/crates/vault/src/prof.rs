//! TEMPORARY profiling (removed before hand-off).
#![allow(missing_docs, clippy::all, clippy::pedantic)]
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static T: Mutex<BTreeMap<&'static str, (Duration, u64)>> = Mutex::new(BTreeMap::new());

pub fn add(k: &'static str, d: Duration) {
    let mut t = T.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let e = t.entry(k).or_default();
    e.0 += d;
    e.1 += 1;
}
pub struct G(&'static str, Instant);
impl Drop for G {
    fn drop(&mut self) {
        add(self.0, self.1.elapsed());
    }
}
pub fn g(k: &'static str) -> G {
    G(k, Instant::now())
}
pub fn reset() {
    T.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
}
pub fn dump(label: &str) {
    let t = T.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for (k, (d, n)) in t.iter() {
        eprintln!("prof[{label}] {k:40} total {:>10.1} ms  n {n:>6}  avg {:>8.3} ms", d.as_secs_f64() * 1e3, d.as_secs_f64() * 1e3 / (*n as f64).max(1.0));
    }
}
