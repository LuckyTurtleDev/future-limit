use std::{
	num::NonZeroUsize,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::sync::Mutex;

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
	pub delay: Duration,
	pub(crate) state: Arc<Mutex<State>>,
}

impl Default for Limiter {
	fn default() -> Self {
		Limiter {
			max_parallelism: None,
			delay: Duration::default(),
			state: Arc::new(Mutex::new(State::default())),
		}
	}
}

pub(crate) enum CanRun<'a> {
	r#True(tokio::sync::MutexGuard<'a, State>),
	r#False(Duration),
}

impl Limiter {
	pub fn new() -> Self {
		Self::default()
	}

	pub(crate) async fn can_run(&self) -> CanRun {
		let state = self.state.lock().await;
		let mut can_run = true;
		let mut wait_duration = Duration::ZERO;
		let time_since_last_run = state.last_run.elapsed();
		if time_since_last_run < self.delay {
			can_run = false;
			wait_duration = wait_duration.max(self.delay - time_since_last_run);
		}
		if let Some(max) = self.max_parallelism {
			if state.current_parallelism >= max.into() {
				can_run = false
			}
		};
		if can_run {
			return CanRun::True(state);
		}
		CanRun::False(wait_duration)
	}
}
