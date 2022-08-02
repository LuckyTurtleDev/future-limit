use std::{future::Future, sync::Arc};
use tokio::{sync::Mutex, task::yield_now};

pub struct Limiter<F>
where
	F: Future,
{
	future: F,
	max_parallelism: usize,
	current_parallelism: Arc<Mutex<usize>>,
}

impl<F> Limiter<F>
where
	F: Future,
{
	pub async fn limit(self) -> <F as Future>::Output {
		loop {
			let mut current_parallelism = self.current_parallelism.lock().await;
			if self.max_parallelism > *current_parallelism {
				*current_parallelism += 1;
				break;
			}
			yield_now().await
		}
		self.future.await
	}
}
