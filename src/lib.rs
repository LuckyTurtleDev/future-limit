use async_trait::async_trait;
use std::{future::Future, sync::Arc, time::Instant};
use tokio::time::sleep;

pub mod limiter;
use limiter::{CanRun, Limiter, YieldStrategie};

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
		for mut state in states {
			state.current_parallelism += 1;
			state.last_run = Instant::now();
			drop(state);
		}
		let return_value = self.await;
		for limit in limits {
			let mut state = limit.state.lock().await;
			state.current_parallelism = state.current_parallelism - 1;
			limit.finish_noftiy.notify_one();
		}
		return_value
	}
}
