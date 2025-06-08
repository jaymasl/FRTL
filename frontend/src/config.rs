use web_sys::window;

pub fn get_api_base_url() -> String {
    // Check if we're running in production (frtl.dev) or locally
    if let Some(window) = window() {
        if let Ok(location) = window.location().host() {
            if location.contains("frtl.dev") {
                // Return empty string for relative URLs when on the production domain
                return "".to_string();
            }
            
            // Use the current hostname and port for API requests
            // This allows the app to work when accessed from other computers
            let protocol = window.location().protocol().unwrap_or_else(|_| "http:".to_string());
            
            // Keep the port number (if any) from the current location
            return format!("{}//{}", protocol, location);
        }
    }
    
    // Default to 127.0.0.1 for development
    "http://127.0.0.1:3000".to_string()
}

pub fn get_asset_url(path: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!("{}{}", get_api_base_url(), path)
    }
} 