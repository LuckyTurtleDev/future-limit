use async_trait::async_trait;
use std::{future::Future, sync::Arc};
use tokio::{sync::Mutex, task::yield_now};

struct LimiterState {
	current_parallelism: usize,
}

pub struct Limiter {
	max_parallelism: usize,
	state: Arc<Mutex<LimiterState>>,
}

#[async_trait]
trait ToName<F>
where
	Self: Future,
{
	async fn limit(self, limits: &Vec<Limiter>) -> <Self as Future>::Output;
}

#[async_trait]
impl<F> ToName<F> for F
where
	F: Future,
	F: Send,
	<Self as Future>::Output: Send,
{
	async fn limit(self, limits: &Vec<Limiter>) -> <Self as Future>::Output {
		let states = loop {
			//look all mutex at the same time
			let mut mutexs = Vec::new();
			for limit in limits {
				match limit.state.try_lock() {
					Err(_) => break,
					Ok(state) => {
						if limit.max_parallelism <= state.current_parallelism {
							break;
						}
						mutexs.push(state)
					},
				}
			}
			if mutexs.len() == limits.len() {
				break mutexs;
			}
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
