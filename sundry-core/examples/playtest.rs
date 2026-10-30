//! Dev harness: runs the real generation pipeline against an external model
//! command (stdin prompt -> stdout JSON). Usage:
//!   SUNDRY_LLM_CMD=path/to/fm cargo run -p sundry-core --example playtest
use std::io::Write;
use std::process::{Command, Stdio};
use sundry_core::climate::{ClimateError, ClimateNormals, ClimateProvider};
use sundry_core::document::TripDocument;
use sundry_core::facts::FactsTable;
use sundry_core::generator::Generator;
use sundry_core::llm::{LlmBackend, LlmError};
use sundry_core::model::*;

struct Kyoto;
#[async_trait::async_trait]
impl ClimateProvider for Kyoto {
    async fn normals(&self, _: String, _: String, _: u32) -> Result<ClimateNormals, ClimateError> {
        Ok(ClimateNormals {
            avg_high_c: 10.4,
            avg_low_c: 1.3,
            precipitation_mm: 72.3,
            daylight_hours: 9.9,
        })
    }
}

struct Cmd(String);
#[async_trait::async_trait]
impl LlmBackend for Cmd {
    async fn generate(&self, prompt: String) -> Result<String, LlmError> {
        let mut child = Command::new(&self.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| LlmError::Failed(e.to_string()))?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(prompt.as_bytes())
            .unwrap();
        let out = child
            .wait_with_output()
            .map_err(|e| LlmError::Failed(e.to_string()))?;
        let raw = String::from_utf8_lossy(&out.stdout).to_string();
        eprintln!("RAW {raw}");
        if !out.status.success() {
            return Err(LlmError::Failed("model failed".into()));
        }
        Ok(raw)
    }
}

fn main() {
    let trip = Trip {
        id: uuid::Uuid::new_v4(),
        title: "Kyoto".into(),
        destinations: vec![Destination {
            country_code: "JP".into(),
            city: "Kyoto".into(),
        }],
        window: TravelWindow::Month {
            year: 2026,
            month: 12,
        },
        nights: 7,
        travellers: vec![
            Traveller {
                id: uuid::Uuid::new_v4(),
                name: "A".into(),
            },
            Traveller {
                id: uuid::Uuid::new_v4(),
                name: "B".into(),
            },
        ],
        purposes: vec![Purpose::Leisure, Purpose::City],
    };
    let facts = FactsTable::load();
    let llm = Cmd(std::env::var("SUNDRY_LLM_CMD").expect("SUNDRY_LLM_CMD"));
    let generator = Generator::new(&facts, &Kyoto, &llm);
    let mut doc = TripDocument::new(&trip);
    let outcome = futures::executor::block_on(generator.generate_all(&mut doc)).unwrap();
    for c in &outcome.per_category {
        println!("{:?}: added {} {:?}", c.category, c.added, c.error);
    }
    let items = doc.items().unwrap();
    for i in &items {
        println!("ITEM [{:?}] {} x{}", i.category, i.name, i.quantity);
    }
    println!("TOTAL {}", items.len());
}
