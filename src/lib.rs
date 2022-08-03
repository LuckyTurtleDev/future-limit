use async_trait::async_trait;
use std::{future::Future, sync::Arc};
use tokio::task::yield_now;

mod limiter;
use limiter::Limiter;

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
			for limit in &limits {
				let state = limit.state.lock().await;
				if limit.max_parallelism.is_none() || state.current_parallelism < limit.max_parallelism.unwrap().into() {
					break;
				}
				mutexs.push(state)
			}
			if mutexs.len() == limits.len() {
				break mutexs;
			}
			drop(mutexs);
			yield_now().await;
		};
		for mut state in states {
			state.current_parallelism += 1;
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
