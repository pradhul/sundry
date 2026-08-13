use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;

pub const MAX_QUANTITY: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedItem {
    pub name: String,
    pub quantity: u32,
    pub rationale: String,
    pub essential: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedList {
    pub items: Vec<GeneratedItem>,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LlmError {
    #[error("the on-device model is unavailable")]
    Unavailable,
    #[error("inference failed: {0}")]
    Failed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("response was not parseable JSON: {0}")]
    Unparseable(String),
    #[error("response contained no usable items")]
    NoUsableItems,
}

/// Canonical form used for duplicate detection, here and in `document.rs`.
pub fn normalise(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn parse_and_validate(raw: &str) -> Result<Vec<GeneratedItem>, ValidationError> {
    let parsed: GeneratedList =
        serde_json::from_str(raw).map_err(|e| ValidationError::Unparseable(e.to_string()))?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut kept = Vec::new();

    for item in parsed.items {
        let name = item.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if item.quantity == 0 || item.quantity > MAX_QUANTITY {
            continue;
        }
        if !seen.insert(normalise(&name)) {
            continue;
        }
        kept.push(GeneratedItem { name, ..item });
    }

    if kept.is_empty() {
        return Err(ValidationError::NoUsableItems);
    }
    Ok(kept)
}

#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync {
    async fn generate(&self, prompt: String) -> Result<String, LlmError>;
}

#[cfg(any(test, feature = "test-support"))]
pub struct FakeLlmBackend {
    responses: Mutex<Vec<String>>,
    fail_with: Option<LlmError>,
}

#[cfg(any(test, feature = "test-support"))]
impl FakeLlmBackend {
    /// Responses are returned in order; the last one repeats once exhausted.
    pub fn replaying(responses: Vec<String>) -> Self {
        FakeLlmBackend {
            responses: Mutex::new(responses),
            fail_with: None,
        }
    }

    pub fn failing(error: LlmError) -> Self {
        FakeLlmBackend {
            responses: Mutex::new(vec![]),
            fail_with: Some(error),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait::async_trait]
impl LlmBackend for FakeLlmBackend {
    async fn generate(&self, _prompt: String) -> Result<String, LlmError> {
        if let Some(err) = &self.fail_with {
            return Err(err.clone());
        }
        let mut queue = self.responses.lock().unwrap();
        if queue.len() > 1 {
            Ok(queue.remove(0))
        } else {
            queue
                .first()
                .cloned()
                .ok_or(LlmError::Failed("no scripted response".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    const GOOD: &str = include_str!("../tests/fixtures/electronics-good.json");
    const MALFORMED: &str = include_str!("../tests/fixtures/electronics-malformed.json");
    const NONSENSE: &str = include_str!("../tests/fixtures/electronics-nonsense.json");

    #[test]
    fn parses_a_good_response() {
        let items = parse_and_validate(GOOD).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Type A plug adapter");
        assert_eq!(items[0].quantity, 2);
        assert!(items[0].essential);
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(
            parse_and_validate(MALFORMED),
            Err(ValidationError::Unparseable(_))
        ));
    }

    #[test]
    fn drops_invalid_items_and_deduplicates() {
        let items = parse_and_validate(NONSENSE).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Charger");
    }

    #[test]
    fn rejects_a_response_with_no_usable_items() {
        let raw = r#"{ "items": [ { "name": "", "quantity": 1, "rationale": "x", "essential": false } ] }"#;
        assert!(matches!(
            parse_and_validate(raw),
            Err(ValidationError::NoUsableItems)
        ));
    }

    #[test]
    fn fake_backend_replays_scripted_responses() {
        let backend = FakeLlmBackend::replaying(vec![GOOD.to_string()]);
        let raw = block_on(backend.generate("any prompt".into())).unwrap();
        assert_eq!(raw, GOOD);
    }
}
