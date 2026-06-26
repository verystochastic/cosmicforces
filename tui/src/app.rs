use std::sync::mpsc;
use std::time::Instant;

use chrono::Utc;
use ratatui::widgets::TableState;
use shared::SolarEvent;
use taivas_calc::calculate_chart;
use taivas_types::{BirthData, CelestialBody, ChartData, HouseSystem, PlanetPosition};

use crate::astro::{self, TwilightStatus};
use crate::guidance;

pub struct Observer {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

impl Observer {
    pub fn from_env() -> Self {
        Observer {
            lat: std::env::var("COSMIC_LAT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(39.1031),
            lon: std::env::var("COSMIC_LON")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(-84.5120),
            name: std::env::var("COSMIC_LOCATION")
                .unwrap_or_else(|_| "Cincinnati".to_string()),
        }
    }
}

pub struct SkyData {
    pub sun_alt: f64,
    pub sun_az: f64,
    pub sun_longitude: f64,
    pub twilight: TwilightStatus,
    pub moon_alt: f64,
    pub moon_az: f64,
    pub moon_longitude: f64,
    pub _moon_phase_angle: f64,
    pub moon_illumination: f64,
    pub moon_phase_name: &'static str,
    pub moon_phase_emoji: &'static str,
    pub sunset: Option<f64>,
    pub sunrise: Option<f64>,
    pub astro_dusk: Option<f64>,
    pub astro_dawn: Option<f64>,
    pub planet_altaz: Vec<(f64, f64)>,
}

impl SkyData {
    pub fn empty() -> Self {
        SkyData {
            sun_alt: 0.0,
            sun_az: 0.0,
            sun_longitude: 0.0,
            twilight: TwilightStatus::Night,
            moon_alt: 0.0,
            moon_az: 0.0,
            moon_longitude: 0.0,
            _moon_phase_angle: 0.0,
            moon_illumination: 0.0,
            moon_phase_name: "—",
            moon_phase_emoji: "☽",
            sunset: None,
            sunrise: None,
            astro_dusk: None,
            astro_dawn: None,
            planet_altaz: Vec::new(),
        }
    }
}

pub struct App {
    pub active_tab: usize,
    pub solar_events: Vec<SolarEvent>,
    pub events_table: TableState,
    pub chart: Option<ChartData>,
    pub sky: SkyData,
    pub observer: Observer,
    pub last_refresh: Instant,
    pub last_sky_update: Instant,
    pub status: String,
    pub natal: Vec<PlanetPosition>,
    pub transit_aspects: Vec<guidance::TransitAspect>,
    pub daily_guidance: guidance::DailyGuidance,
    pub guidance_scroll: u16,
    pub guidance_tx: mpsc::Sender<Result<String, String>>,
    pub guidance_rx: mpsc::Receiver<Result<String, String>>,
}

impl App {
    pub fn new() -> Self {
        let (guidance_tx, guidance_rx) = mpsc::channel();

        // Birth chart: Painesville OH, Nov 21 1972, 12:00 noon EST (= 17:00 UTC)
        let birth_dt = "1972-11-21T17:00:00Z"
            .parse::<chrono::DateTime<Utc>>()
            .expect("valid birth datetime");
        let natal = calculate_chart(
            BirthData {
                datetime: birth_dt,
                latitude: 41.7234,    // Painesville, OH
                longitude: -81.2437,
                timezone: "America/New_York".to_string(),
            },
            HouseSystem::Placidus,
        )
        .map(|c| c.planets)
        .unwrap_or_else(|_| guidance::scott_natal());

        App {
            active_tab: 0,
            solar_events: Vec::new(),
            events_table: TableState::default(),
            chart: None,
            sky: SkyData::empty(),
            observer: Observer::from_env(),
            last_refresh: Instant::now(),
            last_sky_update: Instant::now(),
            status: "Loading...".to_string(),
            natal,
            transit_aspects: Vec::new(),
            daily_guidance: guidance::DailyGuidance::new(),
            guidance_scroll: 0,
            guidance_tx,
            guidance_rx,
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % 4;
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = (self.active_tab + 3) % 4;
    }

    pub fn scroll_down(&mut self) {
        match self.active_tab {
            0 => { self.guidance_scroll = self.guidance_scroll.saturating_add(1); }
            1 => {
                let len = self.solar_events.len();
                if len == 0 { return; }
                let i = self.events_table.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.events_table.select(Some(i));
            }
            _ => {}
        }
    }

    pub fn scroll_up(&mut self) {
        match self.active_tab {
            0 => { self.guidance_scroll = self.guidance_scroll.saturating_sub(1); }
            1 => {
                let len = self.solar_events.len();
                if len == 0 { return; }
                let i = self.events_table.selected()
                    .map(|i| if i == 0 { len - 1 } else { i - 1 })
                    .unwrap_or(0);
                self.events_table.select(Some(i));
            }
            _ => {}
        }
    }

    /// Full refresh: fetch solar events + recompute sky + trigger guidance if stale.
    pub fn refresh(&mut self) {
        self.fetch_solar_events();
        self.compute_sky();
        self.compute_transit_aspects();
        self.trigger_daily_guidance();
        self.last_refresh = Instant::now();
    }

    /// Sky-only update (cheap, no network).
    pub fn update_sky(&mut self) {
        self.compute_sky();
        self.compute_transit_aspects();
        self.last_sky_update = Instant::now();
    }

    fn compute_transit_aspects(&mut self) {
        if let Some(ref chart) = self.chart {
            self.transit_aspects = guidance::find_transit_aspects(&chart.planets, &self.natal);
        }
    }

    pub fn trigger_daily_guidance(&mut self) {
        if matches!(self.daily_guidance.status, guidance::GuidanceStatus::Loading) {
            return;
        }
        if !guidance::should_regenerate(&self.daily_guidance.cache) {
            self.daily_guidance.status = guidance::GuidanceStatus::Ready;
            return;
        }

        let transit_planets = self.chart.as_ref()
            .map(|c| c.planets.clone())
            .unwrap_or_default();

        let has_x_flare = self.solar_events.first()
            .map(|e| e.intensity.starts_with('X'))
            .unwrap_or(false);
        let mercury_rx = transit_planets.iter()
            .find(|p| p.body == CelestialBody::Mercury)
            .map(|p| p.retrograde)
            .unwrap_or(false);

        let prompt = guidance::build_daily_prompt(
            &transit_planets,
            &self.natal,
            &self.transit_aspects,
            self.sky.moon_phase_name,
            self.sky.moon_illumination,
            mercury_rx,
            has_x_flare,
            self.sky.twilight.obs_quality(),
        );

        self.daily_guidance.status = guidance::GuidanceStatus::Loading;
        let tx = self.guidance_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(guidance::call_ollama_blocking(prompt));
        });
    }

    fn fetch_solar_events(&mut self) {
        match reqwest::blocking::get("http://localhost:8080/api/events") {
            Ok(resp) => match resp.json::<Vec<SolarEvent>>() {
                Ok(mut events) => {
                    // most recent first
                    events.sort_by(|a, b| b.peak_time.cmp(&a.peak_time));
                    self.solar_events = events;
                    self.status = format!(
                        "{} solar events  •  observer: {}",
                        self.solar_events.len(),
                        self.observer.name
                    );
                }
                Err(e) => {
                    self.status = format!("Parse error: {e}");
                }
            },
            Err(_) => {
                self.status = format!(
                    "Backend offline — sky data live  •  observer: {}",
                    self.observer.name
                );
            }
        }
    }

    fn compute_sky(&mut self) {
        let now = Utc::now();
        let jd = astro::julian_date(&now);
        let lat = self.observer.lat;
        let lon = self.observer.lon;

        let birth_data = BirthData {
            datetime: now,
            latitude: lat,
            longitude: lon,
            timezone: "UTC".to_string(),
        };

        match calculate_chart(birth_data, HouseSystem::Placidus) {
            Ok(chart) => {
                let sun_lon = chart.sun().map(|p| p.longitude).unwrap_or(0.0);
                let sun_lat = chart.sun().map(|p| p.latitude).unwrap_or(0.0);
                let (sun_alt, sun_az) = astro::body_altitude(sun_lon, sun_lat, jd, lat, lon);

                let moon_lon = chart.moon().map(|p| p.longitude).unwrap_or(0.0);
                let moon_lat = chart.moon().map(|p| p.latitude).unwrap_or(0.0);
                let (moon_alt, moon_az) = astro::body_altitude(moon_lon, moon_lat, jd, lat, lon);

                let phase = astro::moon_phase_angle(sun_lon, moon_lon);

                let (sunrise, sunset) = astro::solar_horizon_times(jd, lat, lon, -0.8333);
                let (astro_dawn, astro_dusk) = astro::solar_horizon_times(jd, lat, lon, -18.0);

                let planet_altaz = chart
                    .planets
                    .iter()
                    .map(|p| astro::body_altitude(p.longitude, p.latitude, jd, lat, lon))
                    .collect();

                self.sky = SkyData {
                    sun_alt,
                    sun_az,
                    sun_longitude: sun_lon,
                    twilight: TwilightStatus::from_sun_altitude(sun_alt),
                    moon_alt,
                    moon_az,
                    moon_longitude: moon_lon,
                    _moon_phase_angle: phase,
                    moon_illumination: astro::moon_illumination(phase),
                    moon_phase_name: astro::moon_phase_name(phase),
                    moon_phase_emoji: astro::moon_phase_emoji(phase),
                    sunset,
                    sunrise,
                    astro_dusk,
                    astro_dawn,
                    planet_altaz,
                };
                self.chart = Some(chart);
            }
            Err(e) => {
                self.status = format!("Calc error: {e}");
            }
        }
    }
}
