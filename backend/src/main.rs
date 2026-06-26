use actix_web::{get, web, App, HttpServer, Responder};
use chrono::{Duration, Utc};
use serde::Deserialize;
use shared::SolarEvent;
use std::{
    sync::Mutex,
    time::{Duration as StdDuration, Instant},
};

#[derive(Debug, Deserialize)]
struct DonkiFlare {
    #[serde(rename = "flrID")]
    _flr_id: String,
    #[serde(rename = "classType")]
    class_type: Option<String>,
    #[serde(rename = "peakTime")]
    peak_time: Option<String>,
    #[serde(rename = "activeRegionNum")]
    active_region_num: Option<i64>,
}

struct AppState {
    events: Mutex<Vec<SolarEvent>>,
    last_fetch: Mutex<Option<Instant>>,
}

const CACHE_TTL: StdDuration = StdDuration::from_secs(300);

async fn fetch_from_donki() -> Vec<SolarEvent> {
    let end = Utc::now();
    let start = end - Duration::days(30);

    let api_key = std::env::var("NASA_API_KEY").unwrap_or_else(|_| "DEMO_KEY".to_string());
    let url = format!(
        "https://api.nasa.gov/DONKI/FLR?startDate={}&endDate={}&api_key={}",
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d"),
        api_key
    );

    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<Vec<DonkiFlare>>().await {
            Ok(flares) => flares
                .into_iter()
                .enumerate()
                .map(|(i, f)| SolarEvent {
                    id: (i + 1).to_string(),
                    event_type: "FLARE".to_string(),
                    peak_time: f.peak_time.unwrap_or_default(),
                    intensity: f.class_type.unwrap_or_else(|| "?".to_string()),
                    active_region: f.active_region_num.map(|n| format!("AR{n}")),
                })
                .collect(),
            Err(e) => {
                eprintln!("DONKI parse error: {e}");
                vec![]
            }
        },
        Err(e) => {
            eprintln!("DONKI fetch error: {e}");
            vec![]
        }
    }
}

#[get("/api/events")]
async fn get_events(data: web::Data<AppState>) -> impl Responder {
    let should_refresh = {
        let last = data.last_fetch.lock().unwrap();
        last.map_or(true, |t| t.elapsed() > CACHE_TTL)
    };

    if should_refresh {
        let fresh = fetch_from_donki().await;
        *data.events.lock().unwrap() = fresh;
        *data.last_fetch.lock().unwrap() = Some(Instant::now());
    }

    let events = data.events.lock().unwrap();
    web::Json(events.clone())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("CosmicForces backend on http://localhost:8080");
    println!("Solar flares fetched from NASA DONKI (cached 5 min)");

    let app_state = web::Data::new(AppState {
        events: Mutex::new(vec![]),
        last_fetch: Mutex::new(None),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(get_events)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
