use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarEvent {
    pub id: String,
    pub event_type: String,  // "FLARE", "CME", etc.
    pub peak_time: String,   // ISO-8601 timestamp
    pub intensity: String,   // "X1.5", "M3.2", etc.
    pub active_region: Option<String>,
}
