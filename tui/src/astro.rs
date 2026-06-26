use chrono::{DateTime, Datelike, Timelike, Utc};

// ── Internal helpers ─────────────────────────────────────────────────────────

fn norm(deg: f64) -> f64 {
    ((deg % 360.0) + 360.0) % 360.0
}

fn jc(jd: f64) -> f64 {
    (jd - 2_451_545.0) / 36_525.0
}

fn obliquity(jd: f64) -> f64 {
    let t = jc(jd);
    23.439_291_111 - 0.013_004_167 * t - 1.64e-7 * t * t + 5.04e-7 * t * t * t
}

fn local_sidereal_time(jd: f64, lon_deg: f64) -> f64 {
    let t = jc(jd);
    let gst = 280.460_618_37
        + 360.985_647_366_29 * (jd - 2_451_545.0)
        + 0.000_387_933 * t * t
        - t * t * t / 38_710_000.0;
    norm(gst + lon_deg)
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn julian_date(dt: &DateTime<Utc>) -> f64 {
    let y = dt.year() as f64;
    let m = dt.month() as f64;
    let d = dt.day() as f64
        + dt.hour() as f64 / 24.0
        + dt.minute() as f64 / 1440.0
        + dt.second() as f64 / 86400.0;
    let (yr, mo) = if m <= 2.0 { (y - 1.0, m + 12.0) } else { (y, m) };
    let a = (yr / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (yr + 4716.0)).floor() + (30.6001 * (mo + 1.0)).floor() + d + b - 1524.5
}

/// Convert ecliptic longitude/latitude to altitude/azimuth for an observer.
pub fn body_altitude(lon_deg: f64, lat_deg: f64, jd: f64, obs_lat: f64, obs_lon: f64) -> (f64, f64) {
    let eps = obliquity(jd).to_radians();
    let lambda = lon_deg.to_radians();
    let beta = lat_deg.to_radians();

    // Ecliptic → equatorial
    let sin_dec = beta.sin() * eps.cos() + beta.cos() * eps.sin() * lambda.sin();
    let ra_y = lambda.sin() * eps.cos() - beta.tan() * eps.sin();
    let ra_x = lambda.cos();
    let ra_deg = norm(ra_y.atan2(ra_x).to_degrees());
    let dec = sin_dec.asin();

    // Equatorial → horizontal
    let lst = local_sidereal_time(jd, obs_lon);
    let ha = norm(lst - ra_deg).to_radians();
    let lat = obs_lat.to_radians();

    let sin_alt = dec.sin() * lat.sin() + dec.cos() * lat.cos() * ha.cos();
    let alt = sin_alt.asin().to_degrees();

    let az_y = -dec.cos() * ha.sin();
    let az_x = dec.sin() * lat.cos() - dec.cos() * lat.sin() * ha.cos();
    let az = norm(az_y.atan2(az_x).to_degrees());

    (alt, az)
}

/// Rise/set times (decimal UTC hours) for a given horizon altitude in degrees.
/// Returns (rise, set). For sunset/rise use -0.8333°; for astro dark use -18°.
pub fn solar_horizon_times(jd: f64, lat_deg: f64, lon_deg: f64, horizon_alt: f64) -> (Option<f64>, Option<f64>) {
    let t = jc(jd);
    let eps = obliquity(jd).to_radians();

    let l0 = 280.466_46 + 36_000.769_83 * t;
    let m_deg = 357.529_11 + 35_999.050_29 * t - 0.000_153_7 * t * t;
    let m_r = m_deg.to_radians();
    let c = (1.914_602 - 0.004_817 * t) * m_r.sin()
        + 0.019_993 * (2.0 * m_r).sin()
        + 0.000_289 * (3.0 * m_r).sin();
    let sun_lon_r = norm(l0 + c).to_radians();

    let sin_dec = eps.sin() * sun_lon_r.sin();
    let cos_dec = (1.0 - sin_dec * sin_dec).sqrt();

    let cos_h = (horizon_alt.to_radians().sin() - lat_deg.to_radians().sin() * sin_dec)
        / (lat_deg.to_radians().cos() * cos_dec);

    if cos_h < -1.0 || cos_h > 1.0 {
        return (None, None);
    }

    let ha_hours = cos_h.acos().to_degrees() / 15.0;
    let noon_utc = 12.0 - lon_deg / 15.0;

    let rise = (noon_utc - ha_hours).rem_euclid(24.0);
    let set = (noon_utc + ha_hours).rem_euclid(24.0);

    (Some(rise), Some(set))
}

pub fn moon_phase_angle(sun_lon: f64, moon_lon: f64) -> f64 {
    norm(moon_lon - sun_lon)
}

pub fn moon_illumination(phase_angle: f64) -> f64 {
    (1.0 - phase_angle.to_radians().cos()) / 2.0 * 100.0
}

pub fn moon_phase_name(phase_angle: f64) -> &'static str {
    match phase_angle as u32 {
        0..=22 => "New Moon",
        23..=67 => "Waxing Crescent",
        68..=112 => "First Quarter",
        113..=157 => "Waxing Gibbous",
        158..=202 => "Full Moon",
        203..=247 => "Waning Gibbous",
        248..=292 => "Last Quarter",
        293..=337 => "Waning Crescent",
        _ => "New Moon",
    }
}

pub fn moon_phase_emoji(phase_angle: f64) -> &'static str {
    match phase_angle as u32 {
        0..=22 => "🌑",
        23..=67 => "🌒",
        68..=112 => "🌓",
        113..=157 => "🌔",
        158..=202 => "🌕",
        203..=247 => "🌖",
        248..=292 => "🌗",
        293..=337 => "🌘",
        _ => "🌑",
    }
}

pub fn compass(az: f64) -> &'static str {
    let a = ((az % 360.0) + 360.0) % 360.0;
    match a as u32 {
        0..=22 | 338..=360 => "N",
        23..=67 => "NE",
        68..=112 => "E",
        113..=157 => "SE",
        158..=202 => "S",
        203..=247 => "SW",
        248..=292 => "W",
        _ => "NW",
    }
}

pub fn format_utc(h: f64) -> String {
    let total = (h * 60.0).round() as i64;
    let hrs = ((total / 60).rem_euclid(24)) as u32;
    let mins = (total % 60 + 60) as u32 % 60;
    format!("{hrs:02}:{mins:02}")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TwilightStatus {
    Day,
    Civil,
    Nautical,
    Astronomical,
    Night,
}

impl TwilightStatus {
    pub fn from_sun_altitude(alt: f64) -> Self {
        if alt > -0.833 {
            TwilightStatus::Day
        } else if alt > -6.0 {
            TwilightStatus::Civil
        } else if alt > -12.0 {
            TwilightStatus::Nautical
        } else if alt > -18.0 {
            TwilightStatus::Astronomical
        } else {
            TwilightStatus::Night
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TwilightStatus::Day => "Daytime",
            TwilightStatus::Civil => "Civil Twilight",
            TwilightStatus::Nautical => "Nautical Twilight",
            TwilightStatus::Astronomical => "Astronomical Twilight",
            TwilightStatus::Night => "Night",
        }
    }

    pub fn obs_quality(self) -> &'static str {
        match self {
            TwilightStatus::Day => "No observing",
            TwilightStatus::Civil => "Poor",
            TwilightStatus::Nautical => "Fair",
            TwilightStatus::Astronomical => "Good",
            TwilightStatus::Night => "Excellent",
        }
    }

    pub fn obs_stars(self) -> &'static str {
        match self {
            TwilightStatus::Day => "✗",
            TwilightStatus::Civil => "★",
            TwilightStatus::Nautical => "★★",
            TwilightStatus::Astronomical => "★★★★",
            TwilightStatus::Night => "★★★★★",
        }
    }
}
