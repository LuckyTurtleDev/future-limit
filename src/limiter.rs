use std::{
	num::NonZeroUsize,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify};

pub(crate) struct State {
	pub(crate) current_parallelism: usize,
	pub(crate) last_run: Instant,
	pub(crate) delay_queued_task: usize,
}

impl Default for State {
	fn default() -> Self {
		State {
			current_parallelism: 0,
			last_run: Instant::now(),
			delay_queued_task: 0,
		}
	}
}

pub(crate) struct StateTasksPerInterval {
	pub(crate) task_count: usize,
	pub(crate) queued_task: usize,
	pub(crate) interval_start: Option<Instant>,
}

impl StateTasksPerInterval {
	fn new() -> Self {
		Self {
			task_count: 0,
			queued_task: 0,
			interval_start: None,
		}
	}
}

pub struct TasksPerInterval {
	pub task_count: NonZeroUsize,
	pub interval: Duration,
	pub(crate) state: Arc<Mutex<StateTasksPerInterval>>,
}

impl TasksPerInterval {
	pub fn new(task_count: NonZeroUsize, interval: Duration) -> Self {
		Self {
			task_count,
			interval,
			state: Arc::new(Mutex::new(StateTasksPerInterval::new())),
		}
	}
}

pub struct Limiter {
	pub max_parallelism: Option<NonZeroUsize>,
	pub(crate) finish_noftiy: Arc<Notify>,
	pub delay: Duration,
	pub(crate) state: Arc<Mutex<State>>,
	pub tasks_per_intervals: Vec<TasksPerInterval>,
}

impl Default for Limiter {
	fn default() -> Self {
		Limiter {
			max_parallelism: None,
			finish_noftiy: Arc::new(Notify::new()),
			delay: Duration::default(),
			state: Arc::new(Mutex::new(State::default())),
			tasks_per_intervals: Vec::new(),
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
			if state.current_parallelism >= max.get() {
				return CanRun::False(YieldStrategie::Notify(&self.finish_noftiy));
			}
			//check if another task can run too, to aviod missing a notify,
			//because permit was already set and not polled yet before it was set again
			if state.current_parallelism + 1 < max.get() {
				self.finish_noftiy.notify_one();
			}
		};
		let mut can_run = true;
		let mut wait_duration = Duration::ZERO;
		let time_since_last_run = state.last_run.elapsed();
		if time_since_last_run < self.delay {
			can_run = false;
			wait_duration =
				wait_duration.max((self.delay - time_since_last_run) + state.delay_queued_task as u32 * self.delay);
		}
		for task_per_interval in &self.tasks_per_intervals {
			let mut state = task_per_interval.state.lock().await;
			if let Some(interval_start) = state.interval_start {
				let time_since_interval_start = interval_start.elapsed();
				if time_since_interval_start > task_per_interval.interval {
					// reset state if interval is over
					state.interval_start = None;
					state.task_count = 0;
				} else {
					if state.task_count >= task_per_interval.task_count.get() {
						can_run = false;
						wait_duration = wait_duration.max(
							task_per_interval.interval - time_since_interval_start
								+ ((state.queued_task / task_per_interval.task_count) as u32 * task_per_interval.interval),
						);
					}
				}
			}
		}
		if can_run {
			return CanRun::True(state);
		}
		CanRun::False(YieldStrategie::Duration(wait_duration))
	}
}

//###########################################  Tests  #############################################
#[cfg(test)]
async fn can_run_task_per_interval(
	runnig_task_count: usize,
	interval: Duration,
	interval_start: Option<Instant>,
	should_run: bool,
) -> () {
	let rule = TasksPerInterval::new(NonZeroUsize::new(10).unwrap(), interval);
	rule.state.lock().await.interval_start = interval_start;
	rule.state.lock().await.task_count = runnig_task_count;
	let mut limit = Limiter::new();
	limit.tasks_per_intervals.push(rule);
	let can_run = match limit.can_run().await {
		CanRun::False(_) => false,
		CanRun::True(_) => true,
	};
	assert_eq!(can_run, should_run);
}

#[cfg(test)]
#[tokio::test]
async fn can_run_task_per_interval_1() -> () {
	can_run_task_per_interval(5, Duration::from_secs(1000), Some(Instant::now()), true).await;
}
#[cfg(test)]
#[tokio::test]
async fn can_run_task_per_interval_2() -> () {
	can_run_task_per_interval(10, Duration::from_secs(1000), Some(Instant::now()), false).await;
}

#[cfg(test)]
#[tokio::test]
async fn can_run_task_per_interval_3() -> () {
	can_run_task_per_interval(10, Duration::from_secs(1000), None, false).await;
}
