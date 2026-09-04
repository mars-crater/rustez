//! e2e: trait contracts — interfaces compile, serde round-trips, usage math holds.
//! Live impls (Qdrant/Proton/SMTP/...) stay in onboarding; here we pin the contracts.

use rustez_channels::{EzChanCtx, EzChannel, EzOutMsg};
use rustez_email::{EzEmail, EzEmailId, EzEmailMsg, EzEmailQuery};
use rustez_images::{EzImage, EzImageGen, EzImagePrompt};
use rustez_memory::{EzMemEntry, EzMemHit, EzMemId, EzMemQuery, EzMemory};
use rustez_secrets::EzSecretStore;
use rustez_usage::{EzSubLimit, EzSubUsage};
use rustez_voice::{EzAudio, EzAudioIn, EzStt, EzTts, EzTtsReq};

struct EzDummyChannel;
impl EzChannel for EzDummyChannel {
    fn id(&self) -> &'static str {
        "dummy"
    }
    async fn start(&self, _ctx: EzChanCtx) -> anyhow::Result<()> {
        Ok(())
    }
    async fn send(&self, _msg: EzOutMsg) -> anyhow::Result<()> {
        Ok(())
    }
}

struct EzDummyMemory;
impl EzMemory for EzDummyMemory {
    fn kind(&self) -> &'static str {
        "dummy"
    }
    async fn put(&self, _e: EzMemEntry) -> anyhow::Result<EzMemId> {
        Ok(EzMemId("1".to_string()))
    }
    async fn search(&self, _q: EzMemQuery) -> anyhow::Result<Vec<EzMemHit>> {
        Ok(vec![])
    }
    async fn get(&self, id: EzMemId) -> anyhow::Result<EzMemEntry> {
        Ok(EzMemEntry {
            text: id.0,
            tags: vec![],
        })
    }
}

struct EzDummyTts;
impl EzTts for EzDummyTts {
    fn kind(&self) -> &'static str {
        "dummy"
    }
    async fn speak(&self, _r: EzTtsReq) -> anyhow::Result<EzAudio> {
        Ok(EzAudio {
            bytes: vec![],
            format: "opus".to_string(),
        })
    }
}

struct EzDummyStt;
impl EzStt for EzDummyStt {
    fn kind(&self) -> &'static str {
        "dummy"
    }
    async fn transcribe(&self, _a: EzAudioIn) -> anyhow::Result<String> {
        Ok(String::new())
    }
}

struct EzDummyImg;
impl EzImageGen for EzDummyImg {
    fn kind(&self) -> &'static str {
        "dummy"
    }
    async fn generate(&self, _p: EzImagePrompt) -> anyhow::Result<EzImage> {
        Ok(EzImage {
            path: "x.png".to_string(),
        })
    }
}

struct EzDummySecrets;
impl EzSecretStore for EzDummySecrets {
    fn kind(&self) -> &'static str {
        "dummy"
    }
    async fn resolve(&self, _id: &str) -> anyhow::Result<String> {
        Ok("s".to_string())
    }
}

struct EzDummyEmail;
impl EzEmail for EzDummyEmail {
    fn kind(&self) -> &'static str {
        "dummy"
    }
    async fn send(&self, _m: EzEmailMsg) -> anyhow::Result<EzEmailId> {
        Ok(EzEmailId("1".to_string()))
    }
    async fn search(&self, _q: EzEmailQuery) -> anyhow::Result<Vec<rustez_email::EzEmailHit>> {
        Ok(vec![])
    }
}

#[test]
fn sub_limit_pct_math() {
    let half = EzSubLimit {
        id: "req".to_string(),
        label: "Requests".to_string(),
        used: 5,
        total: 10,
    };
    assert!((half.pct() - 50.0).abs() < f64::EPSILON);
    let zero = EzSubLimit {
        id: "z".to_string(),
        label: "Z".to_string(),
        used: 1,
        total: 0,
    };
    assert_eq!(zero.pct(), 0.0);
    let over = EzSubLimit {
        id: "o".to_string(),
        label: "O".to_string(),
        used: 12,
        total: 10,
    };
    assert_eq!(over.pct(), 100.0);
    let snap = EzSubUsage {
        provider: "openai".to_string(),
        fetched_at: "now".to_string(),
        limits: vec![half],
        note: String::new(),
    };
    assert_eq!(snap.limits.len(), 1);
}

#[test]
fn secret_and_allowlist_serde() {
    let s: rustez_config::EzSecretInput =
        serde_json::from_str(r#"{"source":"exec","provider":"proton","id":"x"}"#).unwrap();
    assert!(matches!(s, rustez_config::EzSecretInput::Ref(_)));
    let lit: rustez_config::EzSecretInput = serde_json::from_str(r#""abc""#).unwrap();
    assert!(matches!(lit, rustez_config::EzSecretInput::Literal(_)));
    let a: rustez_config::EzAllowFrom = serde_json::from_str(r#"123"#).unwrap();
    assert!(matches!(a, rustez_config::EzAllowFrom::Id(123)));
    let b: rustez_config::EzAllowFrom = serde_json::from_str(r#""owner""#).unwrap();
    assert!(matches!(b, rustez_config::EzAllowFrom::Name(_)));
}

#[tokio::test]
async fn dummy_impls_run() {
    assert_eq!(EzDummyChannel.id(), "dummy");
    EzDummyChannel
        .send(EzOutMsg {
            peer: "p".to_string(),
            text: "hi".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(EzDummyMemory.kind(), "qdrant".replace("qdrant", "dummy"));
    assert_eq!(EzDummyTts.kind(), "dummy");
    assert_eq!(EzDummyStt.kind(), "dummy");
    assert_eq!(EzDummyImg.kind(), "dummy");
    assert_eq!(EzDummySecrets.kind(), "dummy");
    assert_eq!(EzDummyEmail.kind(), "dummy");
}
