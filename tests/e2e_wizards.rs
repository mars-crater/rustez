//! e2e: wizards — every supported impl exposes auth + config steps.

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

const SUPPORTED: &[&str] = &[
    "discord",
    "openai",
    "opencode-go",
    "chutes",
    "qdrant",
    "proton-pass",
    "email",
];

#[tokio::test]
async fn wizards_carry_auth_and_config() {
    for key in SUPPORTED {
        let app = rustez_gateway::router();
        let req = Request::builder()
            .uri(format!("/api/wizards/{key}"))
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{key}");
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["supported"], true, "{key}");
        let steps = v["steps"].as_array().expect("steps array");
        assert_eq!(steps.len(), 2, "{key} must have auth+config steps");
        let auth_fields = steps[0]["fields"].as_array().expect("auth fields");
        assert!(
            auth_fields.iter().any(|f| f["auth"] == true),
            "{key} auth step needs an auth field"
        );
        assert!(
            auth_fields.iter().any(|f| f["env_hint"].is_string()),
            "{key} auth field needs env_hint"
        );
        let cfg_fields = steps[1]["fields"].as_array().expect("config fields");
        assert!(
            cfg_fields.iter().any(|f| f["auth"] == false),
            "{key} config step needs a config field"
        );
    }
}

#[tokio::test]
async fn unsupported_impl_flagged() {
    let app = rustez_gateway::router();
    let req = Request::builder()
        .uri("/api/wizards/telegram")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["supported"], false);
}
