use dioxus::prelude::*;
use shared::SolarEvent;

fn main() {
    launch(App);
}

fn App() -> Element {
    let mut events = use_signal(Vec::<SolarEvent>::new);

    use_effect(move || {
        spawn(async move {
            if let Ok(fetched) = fetch_events().await {
                events.set(fetched);
            }
        });
    });

    rsx! {
        div { class: "min-h-screen bg-gray-900 text-white",
            div { class: "container mx-auto p-4",
                h1 { class: "text-4xl font-bold mb-6", "🌌 CosmicForces" }
                
                div { class: "grid grid-cols-1 gap-4",
                    for event in events.read().iter() {
                        EventCard { event: event.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn EventCard(event: SolarEvent) -> Element {
    let intensity_color = match event.intensity.chars().next() {
        Some('X') => "bg-red-600",
        Some('M') => "bg-orange-500",
        _ => "bg-yellow-400",
    };

    rsx! {
        div { class: "p-4 rounded-lg shadow-md border border-gray-700",
            div { class: "flex items-center gap-3 mb-2",
                div { 
                    class: "rounded-full w-3 h-3 {intensity_color}",
                    title: "{event.intensity} class flare"
                }
                h2 { class: "text-xl font-semibold", 
                    "{event.event_type} • {event.intensity}"
                }
            }
            p { class: "text-gray-300", "{event.peak_time}" }
            if let Some(region) = &event.active_region {
                p { class: "text-sm text-gray-400", "Active Region: {region}" }
            }
        }
    }
}

async fn fetch_events() -> Result<Vec<SolarEvent>, reqwest::Error> {
    reqwest::get("http://localhost:8080/api/events")
        .await?
        .json()
        .await
}
