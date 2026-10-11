use std::num::NonZeroUsize;
use std::sync::LazyLock;

use tokio::runtime::Runtime;

pub static GAME_RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(get_available_cpu_count())
        .thread_name("game")
        .enable_all()
        .build()
        .expect("the game runtime starts")
});

/// At least one.
pub fn get_available_cpu_count() -> usize {
    std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::task::JoinHandle;

    #[test]
    fn game_runtime_runs_a_spawned_task_with_timers() {
        let join_handle: JoinHandle<u32> = GAME_RUNTIME.spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

            7
        });

        assert_eq!(GAME_RUNTIME.block_on(join_handle).unwrap(), 7);
    }
}
