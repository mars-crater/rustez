//! e2e: config — file parsing, defaults, secret masking over HTTP.
//! Single test fn to keep `EZ_CONFIG_PATH` mutation serial within this target.

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn config_defaults_and_masking() {
    // 1. Missing file -> defaults (pure loader, no env).
    let cfg =
        rustez_config::rustez_load("/nonexistent-rustez-e2e-xyz.json").expect("missing -> default");
    assert_eq!(cfg.gateway.port, 18790);
    assert_eq!(cfg.gateway.bind, "loopback");
    assert!(cfg.channels.discord.is_none());

    // 2. HTTP masking: Literal secrets must never leak.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rustez.json");
    std::fs::write(
        &path,
        r#"{"gateway":{"port":18791,"mode":"local","bind":"loopback","auth":{"mode":"token"}},
        "channels":{"discord":{"enabled":true,"token":"LIVESECRET","dmPolicy":"pairing"}}}"#,
    )
    .unwrap();
    std::env::set_var("EZ_CONFIG_PATH", &path);

    let app = rustez_gateway::router();
    let req = Request::builder()
        .uri("/api/config")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        !text.contains("LIVESECRET"),
        "secret literal leaked via /api/config"
    );
    assert!(text.contains("***"), "masked placeholder missing");
    assert!(text.contains("18791"), "configured port not reflected");

    std::env::remove_var("EZ_CONFIG_PATH");
}
