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
use std::time::Duration;
#[cfg(not(test))]
use std::time::Instant;

// Instant is Copy, so there should be no problems with this also being Copy
#[derive(Debug, Clone, Copy)]
enum ExpiredState {
    NotExpired(Instant),
    Expired,
}

impl ExpiredState {
    #[inline]
    const fn is_expired(&self) -> bool {
        matches!(self, Self::Expired)
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
    expired: Cell<ExpiredState>,
    inner: Option<T>,
    max_time: Duration,
}

// Local functions
impl<T> EphemeralOption<T> {
    fn check_time(&self) {
        if let ExpiredState::NotExpired(start_time) = self.expired.get() {
            let cur_time = Instant::now();
            if cur_time.duration_since(start_time) > self.max_time {
                self.expired.set(ExpiredState::Expired);
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
            expired: Cell::new(ExpiredState::new_not_expired()),
            inner: Some(val),
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
            expired: Cell::new(ExpiredState::Expired),
            inner: None,
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
        if self.expired.get().is_expired() {
            return None;
        }
        self.inner.as_ref()
    }

    /// Get a shared reference to the value of the `EphemeralOption`
    /// regardless of whether it has expired or not.
    ///
    /// Will only return `None` if the value does not exist.
    pub const fn get_expired(&self) -> Option<&T> {
        // Don't unnecessarily check time because it isn't used here
        self.inner.as_ref()
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
        if self.expired.get().is_expired() {
            return None;
        }
        self.inner.as_mut()
    }

    /// Get a mutable, exclusive reference to the value of the `EphemeralOption`
    /// regardless of whether it has expired or not.
    ///
    /// Will only return `None` if the value does not exist.
    pub fn get_mut_expired(&mut self) -> Option<&mut T> {
        // Don't unnecessarily check time because it isn't used here
        self.inner.as_mut()
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
        self.expired.set(ExpiredState::new_not_expired());
        self.inner.insert(val)
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
        // Return value if not expired and value exists
        if !self.expired.get().is_expired() {
            if let Some(ref mut inner) = self.inner {
                return inner;
            }
        }
        // Otherwise set new value (use insert because it would be the same implementation anyway)
        self.insert(val)
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
        // Remove value if expired
        if self.expired.get().is_expired() {
            self.inner = None;
        }
        self.inner.take()
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
        // Remove value if expired
        if self.expired.get().is_expired() {
            self.inner = None;
        }
        self.expired.set(ExpiredState::new_not_expired());
        self.inner.replace(val)
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
    // Should &mut self be used here? It isn't required (Cell), but makes more sense API-wise.
    pub fn reset_timer(&mut self) {
        self.expired.set(ExpiredState::new_not_expired());
    }

    /// Convert an `EphemeralOption<T>` into an `Option<T>`.
    /// The `Option` will be `Some(T)` only if the value exists and has not expired,
    /// otherwise it will be `None`.
    pub fn into_option(mut self) -> Option<T> {
        self.check_time();
        if self.expired.get().is_expired() {
            self.inner = None;
        }
        self.inner
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
