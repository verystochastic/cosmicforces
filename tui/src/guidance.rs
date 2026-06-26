use taivas_types::{AspectType, CelestialBody, PlanetPosition};

// ── Transit aspects ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct TransitAspect {
    pub transit_body: CelestialBody,
    pub aspect_type: AspectType,
    pub natal_body: CelestialBody,
    pub orb: f64,
}

pub fn find_transit_aspects(
    transits: &[PlanetPosition],
    natal: &[PlanetPosition],
) -> Vec<TransitAspect> {
    const ASPECT_DEFS: [(AspectType, f64, f64); 5] = [
        (AspectType::Conjunction, 0.0, 8.0),
        (AspectType::Sextile, 60.0, 6.0),
        (AspectType::Square, 90.0, 8.0),
        (AspectType::Trine, 120.0, 6.0),
        (AspectType::Opposition, 180.0, 8.0),
    ];

    let mut aspects = Vec::new();
    for t in transits {
        for n in natal {
            let diff = (t.longitude - n.longitude).abs();
            let sep = if diff > 180.0 { 360.0 - diff } else { diff };
            for &(kind, angle, orb_limit) in &ASPECT_DEFS {
                let orb = (sep - angle).abs();
                if orb <= orb_limit {
                    aspects.push(TransitAspect {
                        transit_body: t.body,
                        aspect_type: kind,
                        natal_body: n.body,
                        orb,
                    });
                }
            }
        }
    }
    aspects.sort_by(|a, b| a.orb.partial_cmp(&b.orb).unwrap_or(std::cmp::Ordering::Equal));
    aspects
}

/// Hardcoded natal positions for Scott (the only user for now).
/// Sun 29°32' Scorpio, Moon 10°46' Gemini, Chiron 13°19' Aries.
pub fn scott_natal() -> Vec<PlanetPosition> {
    vec![
        PlanetPosition::new(CelestialBody::Sun,    239.53, 0.0, 0.0),
        PlanetPosition::new(CelestialBody::Moon,    70.77, 0.0, 0.0),
        PlanetPosition::new(CelestialBody::Chiron,  13.32, 0.0, 0.0),
    ]
}

// ── Cache ─────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GuidanceCache {
    pub date: chrono::NaiveDate,
    pub text: String,
}

pub enum GuidanceStatus {
    Idle,
    Loading,
    Ready,
    Error(String),
}

pub struct DailyGuidance {
    pub cache: Option<GuidanceCache>,
    pub status: GuidanceStatus,
}

impl DailyGuidance {
    pub fn new() -> Self {
        Self { cache: load_cache(), status: GuidanceStatus::Idle }
    }
}

fn cache_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut path = std::path::PathBuf::from(home);
    path.push(".local/share/cosmicforces");
    std::fs::create_dir_all(&path).ok();
    path.push("guidance_cache.json");
    path
}

pub fn load_cache() -> Option<GuidanceCache> {
    let text = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_cache(cache: &GuidanceCache) {
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(cache_path(), json);
    }
}

pub fn should_regenerate(cache: &Option<GuidanceCache>) -> bool {
    match cache {
        None => true,
        Some(c) => c.date != chrono::Local::now().date_naive(),
    }
}

// ── Prompt builder ────────────────────────────────────────────────────────────

pub fn build_daily_prompt(
    transit_planets: &[PlanetPosition],
    natal_planets: &[PlanetPosition],
    aspects: &[TransitAspect],
    moon_phase: &str,
    moon_illumination: f64,
    mercury_rx: bool,
    has_x_flare: bool,
    obs_quality: &str,
) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d");

    let sky_lines: String = transit_planets.iter().map(|p| {
        let rx = if p.retrograde { " (retrograde)" } else { "" };
        if p.body == CelestialBody::Moon {
            format!("  - Moon: {:.1}° {} — {moon_phase}, {moon_illumination:.0}% illuminated{rx}",
                p.degree_in_sign, p.sign)
        } else {
            format!("  - {}: {:.1}° {}{rx}", p.body.name(), p.degree_in_sign, p.sign)
        }
    }).collect::<Vec<_>>().join("\n");

    let natal_lines: String = natal_planets.iter().map(|p| {
        format!("  - {}: {:.1}° {}", p.body.name(), p.degree_in_sign, p.sign)
    }).collect::<Vec<_>>().join("\n");

    let aspect_kind = |a: &TransitAspect| match a.aspect_type {
        AspectType::Conjunction => "conjunct",
        AspectType::Sextile    => "sextile",
        AspectType::Square     => "square",
        AspectType::Trine      => "trine",
        AspectType::Opposition => "opposite",
    };
    let aspect_lines: String = if aspects.is_empty() {
        "  - No major aspects within orb today".to_string()
    } else {
        aspects.iter().take(8).map(|a| {
            format!("  - Transit {} {} natal {} (orb {:.1}°)",
                a.transit_body.name(), aspect_kind(a), a.natal_body.name(), a.orb)
        }).collect::<Vec<_>>().join("\n")
    };

    let mut conditions = Vec::new();
    if mercury_rx { conditions.push("Mercury is retrograde".to_string()); }
    if has_x_flare { conditions.push("X-class solar flare active".to_string()); }
    conditions.push(format!("Sky observing conditions: {obs_quality}"));
    let conditions_lines = conditions.iter().map(|s| format!("  - {s}")).collect::<Vec<_>>().join("\n");

    format!(
        r#"You are an expert astrological interpreter combining psychological depth with
practical daily guidance. Help the user understand what today's sky means specifically
for them, given their natal chart. Be concrete — name actual signs and degrees.
Avoid generic astrology clichés. Reference the user's natal placements when explaining
why a transit matters to them personally.

NATAL CHART (Scott):
  - Sun: 29.5° Scorpio
  - Moon: 10.8° Gemini
  - Ascendant: 24.1° Aquarius
  - Chiron: 13.3° Aries
{natal_lines}

TODAY ({today}) — CURRENT SKY:
{sky_lines}

CURRENT CONDITIONS:
{conditions_lines}

TRANSITS TO NATAL (tightest orb first):
{aspect_lines}

Write 3-4 distinct insights for Scott today. Each insight must:
1. Start with a bracketed label (e.g. [Moon in Taurus] or [Mars–Chiron tension])
2. Explain what this placement means for Scott given his natal chart specifically
3. Offer one concrete thing to notice, feel into, or act on today

Keep each insight to 2-3 sentences. Plain prose, no bullet points within insights."#,
        today = today,
        natal_lines = natal_lines,
        sky_lines = sky_lines,
        conditions_lines = conditions_lines,
        aspect_lines = aspect_lines,
    )
}

// ── Ollama call ───────────────────────────────────────────────────────────────

pub fn call_ollama_blocking(prompt: String) -> Result<String, String> {
    let url = std::env::var("OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = std::env::var("OLLAMA_MODEL")
        .unwrap_or_else(|_| "mistral".to_string());

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(format!("{url}/api/generate"))
        .json(&serde_json::json!({ "model": model, "prompt": prompt, "stream": false }))
        .send()
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Parse error: {e}"))?;

    json["response"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No response field in Ollama output".to_string())
}
