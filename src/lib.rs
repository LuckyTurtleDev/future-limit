use async_trait::async_trait;
use std::{future::Future, sync::Arc, time::Instant};
use tokio::time::sleep;

pub mod limiter;
use limiter::{CanRun, Limiter, YieldStrategie};

#[cfg(test)]
mod test;

#[async_trait]
pub trait LimitFuture<F>
where
	Self: Future,
{
	async fn limits(self, limits: &Vec<&Limiter>) -> <Self as Future>::Output;
	async fn limit(self, limit: &Limiter) -> <Self as Future>::Output;
}

#[async_trait]
impl<F> LimitFuture<F> for F
where
	F: Future,
	F: Send,
	<Self as Future>::Output: Send,
{
	async fn limit(self, limit: &Limiter) -> <Self as Future>::Output {
		self.limits(&vec![&limit]).await
	}

	async fn limits(self, limits: &Vec<&Limiter>) -> <Self as Future>::Output {
		let states = loop {
			let mut limits = limits.clone();
			//sort mutexs by address to avoid deadlocks
			limits.sort_by_key(|a| Arc::as_ptr(&a.state) as usize);
			let mut mutexs = Vec::with_capacity(limits.len());
			let mut yield_strategie = None;
			for limit in &limits {
				match limit.can_run().await {
					CanRun::True(state) => mutexs.push(state),
					CanRun::False(strategie) => {
						yield_strategie = Some(strategie);
						break;
					},
				}
			}
			//check if all limiter were ready
			match yield_strategie {
				None => break mutexs,
				Some(strategie) => {
					drop(mutexs);
					match strategie {
						YieldStrategie::Duration(duration) => sleep(duration).await,
						YieldStrategie::Notify(notify) => notify.notified().await,
					}
				},
			}
		};
		// increment limiters
		for limit in limits {
			for tasks_per_interval in &limit.tasks_per_intervals {
				let mut state = tasks_per_interval.state.lock().await;
				state.task_count += 1;
				if state.interval_start.is_none() {
					state.interval_start = Some(Instant::now());
				}
			}
		}
		for mut state in states {
			state.current_parallelism += 1;
			state.last_run = Instant::now();
			state.delay_queued_task += 1;
			drop(state);
		}
		// run future
		let return_value = self.await;
		// deincrement limiters
		for limit in limits {
			let mut state = limit.state.lock().await;
			state.current_parallelism = state.current_parallelism - 1;
			state.delay_queued_task = 0;
			limit.finish_noftiy.notify_one();
		}
		return_value
	}
}
