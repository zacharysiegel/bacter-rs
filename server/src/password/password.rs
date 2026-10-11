use std::sync::LazyLock;

use actix_web::error::BlockingError;
use actix_web::web;
use bcrypt::BcryptError;
use tokio::sync::{Semaphore, SemaphorePermit};

use shared::error::{AppError, AppErrorStatic};
use shared::protocol::RejectionKind;

use crate::game::game_runtime;

static PASSWORD_CHECK_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(game_runtime::get_available_cpu_count()));

/// Singularity's cost.
const BCRYPT_COST: u32 = 10;
/// Per connection.
pub const PASSWORD_FAILURE_LIMIT: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordMatchKind {
    Matches,
    DoesNotMatch,
}

pub async fn hash_password(password: String) -> Result<String, RejectionKind> {
    run_password_check(&PASSWORD_CHECK_SEMAPHORE, move || {
        bcrypt::hash(password, BCRYPT_COST).map_err(|error| create_bcrypt_error("cannot hash the password", error))
    })
    .await
}

pub async fn verify_password(password: String, password_hash: String) -> Result<PasswordMatchKind, RejectionKind> {
    run_password_check(&PASSWORD_CHECK_SEMAPHORE, move || {
        let matches: bool = bcrypt::verify(password, &password_hash)
            .map_err(|error| create_bcrypt_error("cannot verify the password", error))?;

        match matches {
            true => Ok(PasswordMatchKind::Matches),
            false => Ok(PasswordMatchKind::DoesNotMatch),
        }
    })
    .await
}

fn create_bcrypt_error(message: &'static str, error: BcryptError) -> AppErrorStatic {
    AppErrorStatic::from(AppError::from_error(message, Box::new(error)))
}

/// `ServerBusy` when every permit is taken or the check fails.
async fn run_password_check<T: Send + 'static>(
    semaphore: &Semaphore,
    check: impl FnOnce() -> Result<T, AppErrorStatic> + Send + 'static,
) -> Result<T, RejectionKind> {
    let Ok(_permit): Result<SemaphorePermit<'_>, _> = semaphore.try_acquire() else {
        return Err(RejectionKind::ServerBusy);
    };

    let check_result: Result<Result<T, AppErrorStatic>, BlockingError> = web::block(check).await;

    match check_result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            log::error!("password check failed; [error={error}]");

            Err(RejectionKind::ServerBusy)
        }
        Err(error) => {
            log::error!("password check did not run; [error={error}]");

            Err(RejectionKind::ServerBusy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn verify_password_matches_only_the_hashed_password() {
        let password_hash: String = hash_password(String::from("open sesame")).await.unwrap();

        assert_ne!(password_hash, "open sesame");
        assert_eq!(
            verify_password(String::from("open sesame"), password_hash.clone()).await,
            Ok(PasswordMatchKind::Matches),
        );
        assert_eq!(
            verify_password(String::from("open sesame "), password_hash).await,
            Ok(PasswordMatchKind::DoesNotMatch),
        );
    }

    #[actix_web::test]
    async fn run_password_check_is_busy_without_a_free_permit() {
        let semaphore: Semaphore = Semaphore::new(0);

        assert_eq!(
            run_password_check(&semaphore, || Ok(1)).await,
            Err(RejectionKind::ServerBusy),
        );
    }

    #[actix_web::test]
    async fn run_password_check_is_busy_when_the_check_fails() {
        let semaphore: Semaphore = Semaphore::new(1);

        assert_eq!(
            run_password_check(&semaphore, || Err::<u32, AppErrorStatic>(AppErrorStatic::new("broken"))).await,
            Err(RejectionKind::ServerBusy),
        );
        assert_eq!(semaphore.available_permits(), 1);
    }
}
