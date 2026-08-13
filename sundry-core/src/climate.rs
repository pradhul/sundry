use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClimateNormals {
    pub avg_high_c: f64,
    pub avg_low_c: f64,
    pub precipitation_mm: f64,
    pub daylight_hours: f64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum ClimateError {
    #[error("climate service unreachable")]
    Unreachable,
    #[error("no climate data for this location")]
    NotFound,
}

#[async_trait::async_trait]
pub trait ClimateProvider: Send + Sync {
    async fn normals(
        &self,
        city: String,
        country_code: String,
        month: u32,
    ) -> Result<ClimateNormals, ClimateError>;
}

#[cfg(any(test, feature = "test-support"))]
pub struct FakeClimateProvider {
    outcome: Result<ClimateNormals, ClimateError>,
}

#[cfg(any(test, feature = "test-support"))]
impl FakeClimateProvider {
    pub fn returning(normals: ClimateNormals) -> Self {
        FakeClimateProvider {
            outcome: Ok(normals),
        }
    }

    pub fn failing() -> Self {
        FakeClimateProvider {
            outcome: Err(ClimateError::Unreachable),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait::async_trait]
impl ClimateProvider for FakeClimateProvider {
    async fn normals(
        &self,
        _city: String,
        _country_code: String,
        _month: u32,
    ) -> Result<ClimateNormals, ClimateError> {
        self.outcome.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn fake_provider_returns_configured_normals() {
        let provider = FakeClimateProvider::returning(ClimateNormals {
            avg_high_c: 17.0,
            avg_low_c: 7.0,
            precipitation_mm: 60.0,
            daylight_hours: 10.0,
        });
        let got = block_on(provider.normals("Kyoto".into(), "JP".into(), 11)).unwrap();
        assert_eq!(got.avg_high_c, 17.0);
    }

    #[test]
    fn fake_provider_can_simulate_failure() {
        let provider = FakeClimateProvider::failing();
        let got = block_on(provider.normals("Kyoto".into(), "JP".into(), 11));
        assert!(matches!(got, Err(ClimateError::Unreachable)));
    }
}
