#![warn(clippy::pedantic, clippy::nursery, rust_2018_idioms)]

//! An ephemeral `Option` for Rust. When created, this `EphemeralOption` takes an expiry time and a value,
//! and the `EphemeralOption` will revert to `None` after the time runs out.
//!
//! ## Example
//! ```
//! use ephemeropt::EphemeralOption;
//!
//! let mut num_opt = EphemeralOption::new(0, std::time::Duration::from_secs(1));
//! loop {
//!     match num_opt.get() {
//!         Some(&num) => println!("{num}"),
//!         None => {
//!             let prev_num = num_opt.get_expired().unwrap();
//!             let num = num_opt.insert(prev_num + 1);
//!             println!("{num}");
//!         }
//!     }
//!     std::thread::sleep(std::time::Duration::from_millis(500));
//! }
//! ```

use std::cell::Cell;
use std::mem::{self, MaybeUninit};
use std::time::{Duration, Instant};

/// An `Option` that automatically reverts to `None` after a certain amount of time
///
/// Note: the value in the `EphemeralOption` is not dropped when time expires,
/// only when it is overwritten or the `EphemeralOption` itself is dropped
#[derive(Debug)]
#[must_use]
pub struct EphemeralOption<T> {
    value_state: Cell<ValueState>,
    inner: MaybeUninit<T>,
    max_time: Duration,
    start_time: Instant,
}

#[derive(Debug, Clone, Copy)]
enum ValueState {
    InTime,
    Expired,
    NoValue,
}

impl ValueState {
    // Convenience functions for the checks done most often

    #[inline]
    const fn value_exists(self) -> bool {
        // The 2 states where it does actually contain a value
        matches!(self, Self::InTime | Self::Expired)
    }

    #[inline]
    const fn in_time(self) -> bool {
        matches!(self, Self::InTime)
    }

    #[inline]
    const fn expired(self) -> bool {
        matches!(self, Self::Expired)
    }
}

impl<T> Drop for EphemeralOption<T> {
    // Specifically drop the value inside of the MaybeUninit
    fn drop(&mut self) {
        if self.value_state.get().value_exists() {
            unsafe { self.inner.assume_init_drop() }
        }
    }
}

// Local functions
impl<T> EphemeralOption<T> {
    fn check_time(&self) {
        if matches!(self.value_state.get(), ValueState::InTime) {
            let cur_time = Instant::now();
            if cur_time.duration_since(self.start_time) > self.max_time {
                self.value_state.set(ValueState::Expired);
            }
        }
    }
}

impl<T> EphemeralOption<T> {
    /// Create a new `EphemeralOption<T>` with a value that expires after a set amount of time.
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let opt = EphemeralOption::new("Hello, World!", Duration::from_secs(2));
    /// ```
    pub fn new(val: T, max_time: Duration) -> Self {
        Self {
            value_state: Cell::new(ValueState::InTime),
            inner: MaybeUninit::new(val),
            max_time,
            start_time: Instant::now(),
        }
    }

    /// Create a new, empty `EphemeralOption<T>` that will expire a value after a set amount of time.
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let opt: EphemeralOption<()> = EphemeralOption::new_empty(Duration::from_secs(2));
    /// ```
    pub fn new_empty(max_time: Duration) -> Self {
        Self {
            value_state: Cell::new(ValueState::NoValue),
            inner: MaybeUninit::uninit(),
            max_time,
            // Having a start time here is actually useless, because it will just get replaced when insert is called
            start_time: Instant::now(),
        }
    }

    /// Get a shared reference to the value of the `EphemeralOption`.
    ///
    /// Will return `None` if it is empty or if it has expired.
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// let opt = EphemeralOption::new(3, Duration::from_secs(2));
    /// assert_eq!(opt.get(), Some(&3));
    /// sleep(Duration::from_secs(2));
    /// assert_eq!(opt.get(), None);
    /// ```
    pub fn get(&self) -> Option<&T> {
        self.check_time();
        if self.value_state.get().in_time() {
            unsafe { Some(self.inner.assume_init_ref()) }
        } else {
            None
        }
    }

    /// Get a shared reference to the value of the `EphemeralOption`
    /// regardless of whether it has expired or not.
    ///
    /// Will only return `None` if the value does not exist.
    pub fn get_expired(&self) -> Option<&T> {
        // Don't unnecessarily check time because it isn't used here (in-time and expired both work for value_exists())
        if self.value_state.get().value_exists() {
            unsafe { Some(self.inner.assume_init_ref()) }
        } else {
            None
        }
    }

    /// Get a shared reference to the value of the `EphemeralOption`
    /// without checking if it exists or not.
    ///
    /// It is almost always a better idea to use `get` or `get_expired` instead of this.
    /// The only benefit of this is that it's a `const fn`, so it can be used in constant expressions.
    ///
    /// # Safety
    /// Calling this function will cause undefined behavior if there is no value inside
    /// of the `EphemeralOption`.
    pub const unsafe fn get_unchecked(&self) -> &T {
        // Don't unnecessarily check time because it isn't used here
        self.inner.assume_init_ref()
    }

    /// Get a mutable, exclusive reference to the value of the `EphemeralOption`.
    ///
    /// Will return `None` if it is empty or if it has expired.
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// let mut opt = EphemeralOption::new("hello", Duration::from_secs(2));
    /// let val = opt.get_mut().unwrap();
    /// assert_eq!(val, &mut "hello");
    /// *val = "world";
    /// assert_eq!(val, &mut "world");
    /// sleep(Duration::from_secs(2));
    /// assert_eq!(opt.get_mut(), None);
    /// ```
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.check_time();
        if self.value_state.get().in_time() {
            unsafe { Some(self.inner.assume_init_mut()) }
        } else {
            None
        }
    }

    /// Get a mutable, exclusive reference to the value of the `EphemeralOption`
    /// regardless of whether it has expired or not.
    ///
    /// Will only return `None` if the value does not exist.
    pub fn get_mut_expired(&mut self) -> Option<&T> {
        // Don't unnecessarily check time because it isn't used here (in-time and expired both work for value_exists())
        if self.value_state.get().value_exists() {
            unsafe { Some(self.inner.assume_init_ref()) }
        } else {
            None
        }
    }

    /// Get an exclusive, mutable reference to the value of the
    /// `EphemeralOption` without checking if it exists or not.
    ///
    /// It is almost always a better idea to use `get_mut` or `get_mut_expired` instead of this.
    ///
    /// # Safety
    /// Calling this function will cause undefined behavior if there is no value inside
    /// of the `EphemeralOption`.
    pub unsafe fn get_mut_unchecked(&mut self) -> &mut T {
        // Don't unnecessarily check time because it isn't used here
        self.inner.assume_init_mut()
    }

    /// Overwrite the value in the `EphemeralOption`.
    /// This will drop the value currently in the `EphemeralOption` if it has expired.
    ///
    /// This resets the timer for when the value expires.
    ///
    /// For convenience, this returns a mutable reference to the inserted value.
    ///
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let mut opt = EphemeralOption::new("hello", Duration::from_secs(2));
    /// opt.insert("world");
    /// assert_eq!(opt.get(), Some(&"world"));
    /// ```
    pub fn insert(&mut self, val: T) -> &mut T {
        // No check_time() here because the value is immediately overwritten and the time reset
        if self.value_state.get().value_exists() {
            unsafe { self.inner.assume_init_drop() }
        }
        self.inner.write(val);
        self.value_state.set(ValueState::InTime);
        self.start_time = Instant::now();

        // Has to exist, because it was just set
        unsafe { self.inner.assume_init_mut() }
    }

    /// Overwrite the value in the `EphemeralOption` if it is currently `None`.
    /// This will drop the value currently in the `EphemeralOption` if it has expired.
    ///
    /// If a new value is inserted, this resets the timer for when it expires.
    ///
    /// For convenience, this returns a mutable reference to the inserted value.
    ///
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let mut opt = EphemeralOption::new("hello", Duration::from_secs(2));
    /// opt.insert("world");
    /// assert_eq!(opt.get(), Some(&"world"));
    /// ```
    pub fn get_or_insert(&mut self, val: T) -> &mut T {
        self.check_time();
        // Overwrite with the new value only if it has expired or there is no value
        // (same thing as not in time)
        if !self.value_state.get().in_time() {
            // Drop if is expired
            if self.value_state.get().expired() {
                unsafe { self.inner.assume_init_drop() }
            }
            self.inner.write(val);
            self.value_state.set(ValueState::InTime);
            self.start_time = Instant::now();
        }

        // Has to exist, because it was just set or is already in time
        unsafe { self.inner.assume_init_mut() }
    }

    /// Take the value out of the `EphemeralOption`, leaving it empty.
    ///
    /// This will drop the value currently in the `EphemeralOption` if it has expired.
    ///
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let mut opt = EphemeralOption::new(5, Duration::from_secs(2));
    /// let num = opt.take();
    /// assert_eq!(num, Some(5));
    /// assert_eq!(opt.take(), None);
    /// ```
    pub fn take(&mut self) -> Option<T> {
        self.check_time();
        if self.value_state.get().value_exists() {
            let mut val = mem::replace(&mut self.inner, MaybeUninit::uninit());
            if self.value_state.get().in_time() {
                // If value is in time, get it and return it
                self.value_state.set(ValueState::NoValue);
                let val = unsafe { val.assume_init() };
                return Some(val);
            } else if self.value_state.get().expired() {
                // If it's expired, drop it and continue on to return None
                self.value_state.set(ValueState::NoValue);
                unsafe { val.assume_init_drop() };
            }
        }
        None
    }

    /// Replaces the value in the `EphemeralOption`, leaving the new value in it, without
    /// deinitializing either one.
    /// This resets the timer for when the value expires.
    ///
    /// This will drop the value currently in the `EphemeralOption` if it has expired.
    ///
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let mut opt = EphemeralOption::new(3.14, Duration::from_secs(2));
    /// let num = opt.replace(2.718);
    /// assert_eq!(num, Some(3.14));
    /// assert_eq!(opt.get(), Some(&2.718));
    /// ```
    pub fn replace(&mut self, val: T) -> Option<T> {
        self.check_time();
        if self.value_state.get().value_exists() {
            let mut val = mem::replace(&mut self.inner, MaybeUninit::new(val));
            self.start_time = Instant::now();
            if self.value_state.get().in_time() {
                // If value is in time, get it and return it
                self.value_state.set(ValueState::InTime);
                let val = unsafe { val.assume_init() };
                return Some(val);
            } else if self.value_state.get().expired() {
                // If it's expired, drop it and continue on to return None
                self.value_state.set(ValueState::InTime);
                unsafe { val.assume_init_drop() };
            }
        }
        None
    }

    /// Convert an `EphemeralOption<T>` into an `Option<T>`
    /// The `Option` will be `Some(T)` only if the value exists and has not expired,
    /// otherwise it will be `None`.
    pub fn into_option(mut self) -> Option<T> {
        self.check_time();
        if self.value_state.get().in_time() {
            // Have to extract val using mem::replace
            let val = mem::replace(&mut self.inner, MaybeUninit::uninit());
            // Also have to set value_state so Drop doesn't cause undefined behavior
            // (If it is expired, it should be dropped automatically when this function is rune)
            self.value_state.set(ValueState::NoValue);
            unsafe { Some(val.assume_init()) }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[derive(PartialEq, Debug)]
    struct DropPrint;

    impl Drop for DropPrint {
        fn drop(&mut self) {
            println!("dropped");
        }
    }

    #[test]
    fn general_test() {
        let opt = EphemeralOption::new("hello", Duration::from_secs(1));
        assert_eq!(opt.get(), Some(&"hello"));
        sleep(Duration::from_secs(1));
        assert_eq!(opt.get(), None);
        assert_eq!(opt.get_expired(), Some(&"hello"));

        let mut opt = EphemeralOption::new(DropPrint, Duration::from_millis(500));
        sleep(Duration::from_millis(500));
        // Should print 'dropped'
        assert_eq!(opt.take(), None);
        opt.insert(DropPrint);
        // Will technically also print dropped (new one created)
        assert_eq!(opt.get(), Some(&DropPrint));
        sleep(Duration::from_millis(500));
        // Should also print dropped
        let opt = opt.into_option();
        assert_eq!(opt, None);
    }
}
