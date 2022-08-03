use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::Mutex;

pub(crate) struct State {
	pub(crate) current_parallelism: usize,
}

impl Default for State {
	fn default() -> Self {
		State { current_parallelism: 0 }
	}
}

pub struct Limiter {
	pub max_parallelism: Option<NonZeroUsize>,
	pub(crate) state: Arc<Mutex<State>>,
}

impl Default for Limiter {
	fn default() -> Self {
		Limiter {
			max_parallelism: None,
			state: Arc::new(Mutex::new(State::default())),
		}
	}
}

impl Limiter {
	pub fn new() -> Self {
		Self::default()
	}
}
