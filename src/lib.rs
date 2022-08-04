use async_trait::async_trait;
use std::{
	future::Future,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::{task::yield_now, time::sleep};

pub mod limiter;
use limiter::{CanRun, Limiter};

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
			let mut duration = Duration::ZERO;
			for limit in &limits {
				match limit.can_run().await {
					CanRun::True(state) => mutexs.push(state),
					CanRun::False(duration_) => {
						duration = duration_;
						break;
					},
				}
			}
			//check if all limiter were ready
			if mutexs.len() == limits.len() {
				break mutexs;
			}
			drop(mutexs);
			if duration.is_zero() {
				yield_now().await
			} else {
				sleep(duration).await;
			}
		};
		for mut state in states {
			state.current_parallelism += 1;
			state.last_run = Instant::now();
			drop(state);
		}
		let return_value = self.await;
		for limit in limits {
			let mut state = limit.state.lock().await;
			state.current_parallelism = state.current_parallelism - 1;
		}
		return_value
	}
}
