use crate::{
	limiter::{Limiter, TasksPerInterval},
	LimitFuture,
};
use futures_util::future::join_all;
use std::{
	num::NonZeroUsize,
	time::{Duration, Instant},
};

async fn do_nothing() -> () {}

async fn simple_task_per_interval(task_count: usize, task_per_interval: NonZeroUsize, interval: Duration) -> () {
	let result = 2 + 2;
	assert_eq!(result, 4);
	let mut limit = Limiter::new();
	limit
		.tasks_per_intervals
		.push(TasksPerInterval::new(task_per_interval, interval));
	let mut fu = Vec::new();
	for _ in 0..task_count {
		fu.push(do_nothing().limit(&limit));
	}
	let time = Instant::now();
	join_all(fu).await;
	let time = time.elapsed();
	let min_time = (task_count / task_per_interval.get()) as u32 * interval;
	println!("time passed: {:?}, min time that should have passed {:?}", time, min_time);
	assert!(time >= min_time);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn simple_task_per_interval_20000_4000_1s() -> () {
	simple_task_per_interval(20000, NonZeroUsize::new(400).unwrap(), Duration::from_secs(1)).await;
}
