use server::config::ServerConfig;
use server::registries::ServerRegistries;
use shared::error::AppError;

#[actix_web::main]
async fn main() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .try_init()
        .map_err(|error| AppError::from_error("cannot start the logger", Box::new(error)))?;

    let config: ServerConfig = ServerConfig::from_environment()?;

    server::run(config, ServerRegistries::production()).await
}
