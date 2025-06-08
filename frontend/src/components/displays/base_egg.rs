use crate::models::Egg;
use js_sys::Date;
use wasm_bindgen::JsValue;

pub fn get_egg_title(egg: &Egg) -> String {
    format!("{} Egg", 
        egg.essence.clone().unwrap_or_else(|| "Unknown".to_string())
    )
}

// Format time in hours, minutes, seconds format
fn format_time(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

pub fn get_egg_description(egg: &Egg) -> String {
    if let Some(incubation_ends_at) = &egg.incubation_ends_at {
        let now = Date::new_0();
        let incubation_ends = Date::new(&JsValue::from_str(incubation_ends_at));
        
        if now.get_time() < incubation_ends.get_time() {
            let duration = (incubation_ends.get_time() - now.get_time()) / 1000.0;
            format!(
                "A {} egg containing a mysterious creature. Incubating for {} more...", 
                egg.essence.clone().unwrap_or_else(|| "mysterious".to_string()),
                format_time(duration.floor() as i64)
            )
        } else {
            format!("A {} egg containing a mysterious creature. Ready to hatch!", 
                egg.essence.clone().unwrap_or_else(|| "mysterious".to_string())
            )
        }
    } else {
        format!("A {} egg containing a mysterious creature.", 
            egg.essence.clone().unwrap_or_else(|| "mysterious".to_string())
        )
    }
}

pub fn get_egg_stats(egg: &Egg) -> Vec<(&'static str, f64, f64, &'static str)> {
    let mut stats = vec![];

    if let Some(incubation_ends_at) = &egg.incubation_ends_at {
        let now = js_sys::Date::new_0();
        let incubation_ends = if incubation_ends_at.contains('T') {
            js_sys::Date::new(&wasm_bindgen::JsValue::from_str(incubation_ends_at))
        } else {
            js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&format!("{}T00:00:00Z", incubation_ends_at)))
        };

        let remaining_ms = incubation_ends.get_time() - now.get_time();
        let seconds_remaining = (remaining_ms / 1000.0).floor() as i64;
        
        if seconds_remaining > 0 {
            // Calculate progress percentage based on total incubation time (23 hours = 82800 seconds)
            let total_incubation_time = 82800.0;
            let elapsed_time = total_incubation_time - seconds_remaining as f64;
            let progress = (elapsed_time / total_incubation_time * 100.0).max(0.0).min(100.0);
            
            // Use a static string for the label instead of a formatted time
            stats.push(("Incubating", progress, 100.0, "bg-gradient-to-r from-yellow-500 to-yellow-600"));
        } else {
            stats.push(("Ready!", 100.0, 100.0, "bg-gradient-to-r from-green-500 to-green-600"));
        }
    }

    stats
}

pub fn get_egg_card_stats(egg: &Egg) -> Vec<(&'static str, f64, f64, &'static str)> {
    let mut stats = vec![];

    if let Some(incubation_ends_at) = &egg.incubation_ends_at {
        let now = js_sys::Date::new_0();
        let incubation_ends = if incubation_ends_at.contains('T') {
            js_sys::Date::new(&wasm_bindgen::JsValue::from_str(incubation_ends_at))
        } else {
            js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&format!("{}T00:00:00Z", incubation_ends_at)))
        };

        let remaining_ms = incubation_ends.get_time() - now.get_time();
        let seconds_remaining = (remaining_ms / 1000.0).floor() as i64;
        
        if seconds_remaining > 0 {
            // Calculate progress percentage based on total incubation time (23 hours = 82800 seconds)
            let total_incubation_time = 82800.0;
            let elapsed_time = total_incubation_time - seconds_remaining as f64;
            let progress = (elapsed_time / total_incubation_time * 100.0).max(0.0).min(100.0);
            
            stats.push(("Incubating", progress, 100.0, "bg-gradient-to-r from-yellow-500 to-yellow-600"));
        } else {
            stats.push(("Ready!", 100.0, 100.0, "bg-gradient-to-r from-green-500 to-green-600"));
        }
    }

    stats
}

pub fn get_egg_details(egg: &Egg) -> Vec<(String, Option<String>)> {
    vec![
        ("Owner".to_string(), egg.owner_username.clone()),
        ("Essence".to_string(), egg.essence.clone()),
        ("Color".to_string(), egg.color.clone()),
        ("Style".to_string(), egg.art_style.clone()),
    ]
}