use std::{
	num::NonZeroUsize,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify};

pub(crate) struct State {
	pub(crate) current_parallelism: usize,
	pub(crate) last_run: Instant,
}

impl Default for State {
	fn default() -> Self {
		State {
			current_parallelism: 0,
			last_run: Instant::now(),
		}
	}
}

pub struct Limiter {
	pub max_parallelism: Option<NonZeroUsize>,
	pub(crate) finish_noftiy: Arc<Notify>,
	pub delay: Duration,
	pub(crate) state: Arc<Mutex<State>>,
}

impl Default for Limiter {
	fn default() -> Self {
		Limiter {
			max_parallelism: None,
			finish_noftiy: Arc::new(Notify::new()),
			delay: Duration::default(),
			state: Arc::new(Mutex::new(State::default())),
		}
	}
}

pub(crate) enum YieldStrategie<'a> {
	Duration(Duration),
	Notify(&'a Arc<Notify>),
}

pub(crate) enum CanRun<'a> {
	r#True(tokio::sync::MutexGuard<'a, State>),
	False(YieldStrategie<'a>),
}

impl Limiter {
	pub fn new() -> Self {
		Self::default()
	}

	pub(crate) async fn can_run(&self) -> CanRun {
		let state = self.state.lock().await;
		if let Some(max) = self.max_parallelism {
			if state.current_parallelism >= max.into() {
				return CanRun::False(YieldStrategie::Notify(&self.finish_noftiy));
			}
		};
		let mut can_run = true;
		let mut wait_duration = Duration::ZERO;
		let time_since_last_run = state.last_run.elapsed();
		if time_since_last_run < self.delay {
			can_run = false;
			wait_duration = wait_duration.max(self.delay - time_since_last_run);
		}
		if can_run {
			return CanRun::True(state);
		}
		CanRun::False(YieldStrategie::Duration(wait_duration))
	}
}
