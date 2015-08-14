extern crate time;

use std::cmp::PartialEq;
use std::sync::{Mutex, Condvar, PoisonError, MutexGuard, LockResult};

pub enum Notify {
	One,
	All,
}

pub struct ConditionVariable<T> {
	pair: (Mutex<T>, Condvar)
}

impl<T:PartialEq+Clone> ConditionVariable<T> {
	pub fn new(value: T) -> ConditionVariable<T> {
		ConditionVariable {
			pair: (Mutex::new(value), Condvar::new())
		}
	}

	pub fn set(&self, value: T, notify: Notify) {
		let &(ref lock, ref cvar) = &self.pair;

		let mut data = lock.lock().unwrap();
		*data = value;

		match notify {
			Notify::One => cvar.notify_one(),
			Notify::All => cvar.notify_all(),
		}
	}

	pub fn get(&self) -> Result<T, PoisonError<MutexGuard<T>>> {
		let &(ref lock, _) = &self.pair;

		let data = try!(lock.lock());

		Ok(data.clone())
	}

	pub fn wait_for(&self, expected: T) -> Result<(), PoisonError<MutexGuard<T>>> {
		self.wait_for_in(&[expected])
	}

	pub fn wait_for_in(&self, expected: &[T]) -> Result<(), PoisonError<MutexGuard<T>>> {
		self.wait_for_condition(|actual| expected.contains(actual))
	}

	pub fn wait_for_condition<F:Fn(&T) -> bool>(&self, cond_func: F) -> Result<(), PoisonError<MutexGuard<T>>> {
		let &(ref lock, ref cvar) = &self.pair;
		let mut actual = try!(lock.lock());
		
		while !cond_func(&*actual) {
			actual = try!(cvar.wait(actual));
		}

		Ok(())
	}

	pub fn wait_for_ms(&self, expected: T, timeout_ms: i64) -> Result<bool, PoisonError<(MutexGuard<T>,bool)>> {
		self.wait_for_in_ms(&[expected], timeout_ms)
	}

	pub fn wait_for_in_ms(&self, expected: &[T], timeout_ms: i64)
		-> Result<bool, PoisonError<(MutexGuard<T>,bool)>>
	{
		self.wait_for_condition_ms(|actual| expected.contains(actual), timeout_ms)
	}

	pub fn wait_for_condition_ms<F:Fn(&T) -> bool>(&self, cond_func: F, timeout_ms: i64)
		-> Result<bool, PoisonError<(MutexGuard<T>,bool)>>
	{
		let &(ref lock, ref cvar) = &self.pair;
		let mut actual = lock.lock().unwrap();

		let mut remaining_ms = timeout_ms;
		while !cond_func(&*actual) && remaining_ms > 0 {
			let before_ms = time::precise_time_ns()/1000;

			let (new, _) = try!(cvar.wait_timeout_ms(actual, remaining_ms as u32));
			actual = new;

			let after_ms = time::precise_time_ns()/1000;
			remaining_ms -= (after_ms - before_ms) as i64;
		}

		Ok(cond_func(&*actual))
	}
}

impl ConditionVariable<()> {
	/// waits for a notify (useful if T==())
	pub fn wait_ms(&self, timeout_ms: u32) -> LockResult<(MutexGuard<()>,bool)>
	{
		let &(ref lock, ref cvar) = &self.pair;
		let guard = lock.lock().unwrap();
		
		cvar.wait_timeout_ms(guard, timeout_ms)
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::thread::{sleep_ms, spawn};

	use ::Notify;
	use ::ConditionVariable;

	#[test]
	fn test_wait_for() {
		let cvar1 = Arc::new(ConditionVariable::new(false));
		let cvar2 = cvar1.clone();

		spawn(move || {
			cvar2.set(true, Notify::All);
		});

		cvar1.wait_for(true).unwrap();
	}

	#[test]
	fn test_wait_for_ms() {
		let cvar1 = Arc::new(ConditionVariable::new(false));
		let cvar2 = cvar1.clone();

		spawn(move || {
			sleep_ms(500);
			cvar2.set(true, Notify::All);
		});

		assert_eq!(cvar1.wait_for_ms(true, 1000).unwrap(), true);
	}

	#[test]
	fn test_wait_for_ms_fail() {
		let cvar1 = Arc::new(ConditionVariable::new(false));
		let cvar2 = cvar1.clone();

		spawn(move || {
			sleep_ms(1000);
			cvar2.set(true, Notify::All);
		});

		assert_eq!(cvar1.wait_for_ms(true, 500).unwrap(), false);
	}
}
