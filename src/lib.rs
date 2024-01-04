#![warn(clippy::pedantic, clippy::nursery, rust_2018_idioms)]

//! An ephemeral `Option` for Rust. When created, this `EphemeralOption` takes an expiry time and a value,
//! and the `EphemeralOption` will revert to `None` after the time runs out.
//!
//! ## Example
//! ```no_run
//! use ephemeropt::EphemeralOption;
//!
//! let mut num_opt = EphemeralOption::new(0, std::time::Duration::from_secs(1));
//! // Will only go up to 10, because every other call will be cached
//! for _ in 0..=20 {
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

#[cfg(test)]
use mock_instant::Instant;
use std::cell::Cell;
use std::mem::{self, MaybeUninit};
use std::time::Duration;
#[cfg(not(test))]
use std::time::Instant;

// Instant is Copy, so there should be no problems with this also being Copy
// Every time it copies out of the cell, it does copy 16 bytes vs. 1 byte
// With modern system performance, this shouldn't matter
#[derive(Debug, Clone, Copy)]
enum ValueState {
    NoValue,
    NotExpired(Instant),
    Expired,
}

impl ValueState {
    // Convenience methods
    #[inline]
    const fn is_not_expired(&self) -> bool {
        matches!(self, Self::NotExpired(_))
    }

    #[inline]
    const fn is_expired(&self) -> bool {
        matches!(self, Self::Expired)
    }

    #[inline]
    const fn is_no_value(&self) -> bool {
        matches!(self, Self::NoValue)
    }

    #[inline]
    const fn exists(&self) -> bool {
        !self.is_no_value()
    }

    fn new_not_expired() -> Self {
        Self::NotExpired(Instant::now())
    }
}

/// An `Option` that automatically reverts to `None` after a certain amount of time
///
/// The value in the `EphemeralOption` is not dropped when time expires,
/// only when it is overwritten or the `EphemeralOption` itself is dropped
#[derive(Debug)]
#[must_use]
pub struct EphemeralOption<T> {
    state: Cell<ValueState>,
    inner: MaybeUninit<T>,
    max_time: Duration,
}

impl<T> Drop for EphemeralOption<T> {
    fn drop(&mut self) {
        // Specifically drop inner value if it exists
        if self.state.get().exists() {
            // SAFETY: just checked that value exists
            unsafe { self.inner.assume_init_drop() }
        }
    }
}

// Local functions
impl<T> EphemeralOption<T> {
    fn check_time(&self) {
        if let ValueState::NotExpired(start_time) = self.state.get() {
            let cur_time = Instant::now();
            if cur_time.duration_since(start_time) > self.max_time {
                self.state.set(ValueState::Expired);
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
            state: Cell::new(ValueState::new_not_expired()),
            inner: MaybeUninit::new(val),
            max_time,
        }
    }

    /// Create a new, empty `EphemeralOption<T>` that will expire a value after a set amount of time.
    /// ```
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// let opt: EphemeralOption<()> = EphemeralOption::new_empty(Duration::from_secs(2));
    /// ```
    pub const fn new_empty(max_time: Duration) -> Self {
        Self {
            state: Cell::new(ValueState::NoValue),
            inner: MaybeUninit::uninit(),
            max_time,
        }
    }

    /// Get a shared reference to the value of the `EphemeralOption`.
    ///
    /// Will return `None` if it is empty or if it has expired.
    /// ```no_run
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

        if self.state.get().is_expired() {
            return None;
        }
        // SAFETY: checked to make sure value isn't expired
        unsafe { Some(self.inner.assume_init_ref()) }
    }

    /// Get a shared reference to the value of the `EphemeralOption`
    /// without checking if it exists or not.
    ///
    /// It is almost always a better idea to use `get` or `get_expired` instead of this.
    ///
    /// # Safety
    /// Calling this function will cause undefined behavior if there is no value inside
    /// of the `EphemeralOption`.
    pub const unsafe fn get_unchecked(&self) -> &T {
        // Don't unnecessarily check time because it isn't used here
        self.inner.assume_init_ref()
    }

    /// Get a shared reference to the value of the `EphemeralOption`
    /// regardless of whether it has expired or not.
    ///
    /// Will only return `None` if the value does not exist.
    pub fn get_expired(&self) -> Option<&T> {
        // Don't unnecessarily check time because it isn't used here
        if self.state.get().is_no_value() {
            return None;
        }
        // SAFETY: checked to make sure value exists
        unsafe { Some(self.inner.assume_init_ref()) }
    }

    /// Get a mutable, exclusive reference to the value of the `EphemeralOption`.
    ///
    /// Will return `None` if it is empty or if it has expired.
    /// ```no_run
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

        if self.state.get().is_expired() {
            return None;
        }
        // SAFETY: checked to make sure value isn't expired
        unsafe { Some(self.inner.assume_init_mut()) }
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

    /// Get a mutable, exclusive reference to the value of the `EphemeralOption`
    /// regardless of whether it has expired or not.
    ///
    /// Will only return `None` if the value does not exist.
    pub fn get_mut_expired(&mut self) -> Option<&mut T> {
        // Don't unnecessarily check time because it isn't used here
        if self.state.get().is_no_value() {
            return None;
        }
        // SAFETY: checked to make sure value exists
        unsafe { Some(self.inner.assume_init_mut()) }
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
        // Make sure that value exists, regardless of whether it's expired
        if self.state.get().exists() {
            // SAFETY: just checked that value exists
            unsafe { self.inner.assume_init_drop() }
        }
        self.state.set(ValueState::new_not_expired());
        self.inner.write(val)
    }

    /// Overwrite the value in the `EphemeralOption` if it is currently `None`.
    /// This will drop the value currently in the `EphemeralOption` if it has expired.
    ///
    /// If a new value is inserted, this resets the timer for when it expires.
    ///
    /// For convenience, this returns a mutable reference to the contained value.
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

        let state = self.state.get();

        // If value isn't expired, immediately return it
        if state.is_not_expired() {
            // SAFETY: value is not expired, therefore has to exist
            return unsafe { self.inner.assume_init_mut() };
        }

        // Otherwise, drop value if it is expired, and insert new one
        if state.is_expired() {
            // SAFETY: though it is expired, value exists
            unsafe { self.inner.assume_init_drop() };
        }
        self.state.set(ValueState::new_not_expired());
        self.inner.write(val)
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

        // Vastly different implementations depending on state, so just match on everything here
        match self.state.get() {
            ValueState::NoValue => None,
            // If value is expired, drop it in place, but still set NoValue state
            ValueState::Expired => {
                self.state.set(ValueState::NoValue);
                // SAFETY: even though value is expired, it exists
                unsafe { self.inner.assume_init_drop() };
                None
            }
            ValueState::NotExpired(_) => {
                let val = mem::replace(&mut self.inner, MaybeUninit::uninit());
                self.state.set(ValueState::NoValue);
                // SAFETY: value isn't expired, therefore has to exist
                let val = unsafe { val.assume_init() };
                Some(val)
            }
        }
    }

    /// Replaces the value in the `EphemeralOption` with the new one and
    /// returns the old value if present, without deinitializing either one.
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

        let state = self.state.get();

        if state.is_not_expired() {
            let old_val = mem::replace(&mut self.inner, MaybeUninit::new(val));
            self.state.set(ValueState::new_not_expired());
            // SAFETY: value isn't expired, therefore has to exist
            let old_val = unsafe { old_val.assume_init() };
            return Some(old_val);
        }

        // Otherwise, drop value if it is expired, and insert new one
        if state.is_expired() {
            // SAFETY: even though value is expired, it exists
            unsafe { self.inner.assume_init_drop() };
        }

        self.state.set(ValueState::new_not_expired());
        self.inner.write(val);

        None
    }

    /// Reset the timer for when the value expires.
    ///
    /// ```no_run
    /// # use ephemeropt::EphemeralOption;
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// let mut opt = EphemeralOption::new(3, Duration::from_secs(2));
    /// sleep(Duration::from_secs(2));
    /// opt.reset_timer();
    /// assert_eq!(opt.get(), Some(&3));
    /// ```
    pub fn reset_timer(&self) {
        // Only reset the timer if the value actually exists
        if self.state.get().exists() {
            self.state.set(ValueState::new_not_expired());
        }
    }

    /// Convert an `EphemeralOption<T>` into an `Option<T>`.
    /// The `Option` will be `Some(T)` only if the value exists and has not expired,
    /// otherwise it will be `None`.
    pub fn into_option(mut self) -> Option<T> {
        self.check_time();

        match self.state.get() {
            ValueState::NoValue => None,
            ValueState::Expired => {
                // SAFETY: even though value is expired, it exists
                unsafe { self.inner.assume_init_drop() };
                None
            }
            ValueState::NotExpired(_) => {
                let val = mem::replace(&mut self.inner, MaybeUninit::uninit());
                // SAFETY: since value isn't expired, it has to exist
                unsafe { Some(val.assume_init()) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_instant::MockClock;

    #[test]
    fn general_test() {
        let opt = EphemeralOption::new("hello", Duration::from_secs(1));
        assert_eq!(opt.get(), Some(&"hello"));
        // Have to advance the clock just past the time for tests to work
        MockClock::advance(Duration::from_millis(1001));
        assert_eq!(opt.get(), None);
        assert_eq!(opt.get_expired(), Some(&"hello"));

        let mut opt = EphemeralOption::new(3, Duration::from_millis(500));
        MockClock::advance(Duration::from_millis(501));

        assert_eq!(opt.take(), None);
        opt.insert(2);
        {
            let num = opt.get_mut().unwrap();
            *num = 1;
        }
        assert_eq!(opt.get(), Some(&1));
        MockClock::advance(Duration::from_millis(501));
        {
            let num = opt.get_mut_expired().unwrap();
            *num = 2;
        }
        opt.reset_timer();
        assert_eq!(opt.replace(3), Some(2));
        assert_eq!(opt.get_or_insert(0), &mut 3);
    }
}
