use std::path::Path;

use actix_files::Files;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header;
use actix_web::http::header::HeaderValue;
use actix_web::middleware::Next;
use actix_web::{HttpResponse, middleware, web};

const INDEX_FILE_NAME: &str = "index.html";
const IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const NO_CACHE_CONTROL: &str = "no-cache";

/// No files without a `site_root`.
pub fn configurer(service_config: &mut web::ServiceConfig, site_root: Option<&Path>) {
    service_config.route("/health", web::get().to(get_health));

    let Some(site_root) = site_root else {
        return;
    };

    service_config.service(
        web::scope("")
            .wrap(middleware::from_fn(set_cache_control))
            .service(Files::new("/", site_root).index_file(INDEX_FILE_NAME)),
    );
}

async fn get_health() -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain; charset=utf-8").body("ok")
}

async fn set_cache_control(
    request: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let cache_control: Option<&'static str> = get_cache_control(request.path());
    let mut response: ServiceResponse<_> = next.call(request).await?;

    if let Some(cache_control) = cache_control {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control));
    }

    Ok(response)
}

/// Hashed `/pkg/` files never change; the entry points must be revalidated.
fn get_cache_control(path: &str) -> Option<&'static str> {
    if path.starts_with("/pkg/") {
        return Some(IMMUTABLE_CACHE_CONTROL);
    }

    match path {
        "/" | "/index.html" | "/game-server.json" => Some(NO_CACHE_CONTROL),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use actix_web::http::StatusCode;
    use actix_web::{App, test};
    use std::path::PathBuf;

    fn create_site_root(name: &str) -> PathBuf {
        let site_root: PathBuf = test_fixture::create_temporary_directory(name);
        std::fs::create_dir_all(site_root.join("pkg")).unwrap();
        std::fs::write(site_root.join("index.html"), "<!doctype html>").unwrap();
        std::fs::write(site_root.join("game-server.json"), "{}").unwrap();
        std::fs::write(site_root.join("pkg/bacter-0a1b.wasm"), "wasm").unwrap();
        std::fs::write(site_root.join("favicon.ico"), "icon").unwrap();

        site_root
    }

    /// The status, cache control header and body of each path.
    async fn get_responses(site_root: Option<&Path>, paths: &[&str]) -> Vec<(StatusCode, Option<String>, String)> {
        let service = test::init_service(App::new().configure(|service_config| {
            configurer(service_config, site_root);
        }))
        .await;
        let mut responses: Vec<(StatusCode, Option<String>, String)> = Vec::new();

        for path in paths {
            let response: ServiceResponse =
                test::call_service(&service, test::TestRequest::get().uri(path).to_request()).await;
            let status: StatusCode = response.status();
            let cache_control: Option<String> =
                response.headers().get(header::CACHE_CONTROL).map(|value| String::from(value.to_str().unwrap()));
            let body: String = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();

            responses.push((status, cache_control, body));
        }

        responses
    }

    #[actix_web::test]
    async fn configurer_serves_the_site_with_its_cache_headers() {
        let site_root: PathBuf = create_site_root("site-cache-headers");
        let no_cache: Option<String> = Some(String::from(NO_CACHE_CONTROL));

        let responses: Vec<(StatusCode, Option<String>, String)> = get_responses(
            Some(&site_root),
            &["/", "/game-server.json", "/pkg/bacter-0a1b.wasm", "/favicon.ico"],
        )
        .await;

        assert_eq!(
            responses,
            vec![
                (StatusCode::OK, no_cache.clone(), String::from("<!doctype html>")),
                (StatusCode::OK, no_cache, String::from("{}")),
                (
                    StatusCode::OK,
                    Some(String::from(IMMUTABLE_CACHE_CONTROL)),
                    String::from("wasm")
                ),
                (StatusCode::OK, None, String::from("icon")),
            ],
        );
    }

    #[actix_web::test]
    async fn configurer_answers_health_with_or_without_a_site() {
        let site_root: PathBuf = create_site_root("site-health");
        let health_response: (StatusCode, Option<String>, String) = (StatusCode::OK, None, String::from("ok"));

        assert_eq!(
            get_responses(Some(&site_root), &["/health"]).await,
            vec![health_response.clone()]
        );
        assert_eq!(get_responses(None, &["/health"]).await, vec![health_response]);
    }

    #[actix_web::test]
    async fn configurer_serves_no_files_without_a_site_root() {
        let responses: Vec<(StatusCode, Option<String>, String)> = get_responses(None, &["/"]).await;

        assert_eq!(responses[0].0, StatusCode::NOT_FOUND);
    }
}
