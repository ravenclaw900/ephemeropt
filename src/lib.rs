#![warn(clippy::pedantic, clippy::nursery, rust_2018_idioms)]

use std::cell::Cell;
use std::mem::{self, MaybeUninit};
use std::time::{Duration, Instant};

#[derive(Debug)]
#[must_use]
pub struct TimedOption<T> {
    exists: Cell<bool>,
    inner: MaybeUninit<T>,
    max_time: Duration,
    start_time: Instant,
}

impl<T> TimedOption<T> {
    pub fn new(val: T, max_time: Duration) -> Self {
        Self {
            exists: Cell::new(true),
            inner: MaybeUninit::new(val),
            max_time,
            start_time: Instant::now(),
        }
    }

    pub fn new_empty(max_time: Duration) -> Self {
        Self {
            exists: Cell::new(false),
            inner: MaybeUninit::uninit(),
            max_time,
            // Having a start time here is actually useless, because it will just get replaced when insert is called
            start_time: Instant::now(),
        }
    }

    fn check_time(&self) {
        let cur_time = Instant::now();
        if cur_time.duration_since(self.start_time) > self.max_time && self.exists.get() {
            self.exists.set(false);
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.check_time();
        if self.exists.get() {
            unsafe { Some(self.inner.assume_init_ref()) }
        } else {
            None
        }
    }

    pub unsafe fn get_unchecked(&self) -> &T {
        self.inner.assume_init_ref()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.check_time();
        if self.exists.get() {
            unsafe { Some(self.inner.assume_init_mut()) }
        } else {
            None
        }
    }

    pub unsafe fn get_mut_unchecked(&mut self) -> &mut T {
        self.inner.assume_init_mut()
    }

    pub fn insert(&mut self, val: T) -> &mut T {
        if self.exists.get() {
            unsafe { self.inner.assume_init_drop() }
        }
        self.inner.write(val);
        self.exists.set(true);
        self.start_time = Instant::now();

        // Has to exist, because it was just set
        unsafe { self.inner.assume_init_mut() }
    }

    pub fn get_or_insert(&mut self, val: T) -> &mut T {
        self.check_time();
        if !self.exists.get() {
            self.inner.write(val);
            self.exists.set(true);
            self.start_time = Instant::now();
        }

        // Has to exist, because it was just set
        unsafe { self.inner.assume_init_mut() }
    }

    pub fn take(&mut self) -> Option<T> {
        self.check_time();
        if self.exists.get() {
            self.exists.set(false);
            let val = mem::replace(&mut self.inner, MaybeUninit::uninit());
            let val = unsafe { val.assume_init() };
            return Some(val);
        }
        None
    }

    pub fn replace(&mut self, val: T) -> Option<T> {
        self.check_time();
        if self.exists.get() {
            self.exists.set(false);
            let val = mem::replace(&mut self.inner, MaybeUninit::new(val));
            let val = unsafe { val.assume_init() };
            return Some(val);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_test() {
        let opt = TimedOption::new("hello", Duration::from_secs(1));
        assert_eq!(opt.get(), Some(&"hello"));
        std::thread::sleep(Duration::from_secs(1));
        assert_eq!(opt.get(), None);
    }
}
