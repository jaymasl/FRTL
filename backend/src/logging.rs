use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, Layer, EnvFilter, Registry};
use tracing_subscriber::prelude::*;
use chrono;

#[derive(Default)]
struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push_str(&format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        }
    }
}

struct CustomLayer;

impl<S: Subscriber> Layer<S> for CustomLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        if metadata.target().contains("sqlx") {
            // Skip SQL query logging
            return;
        }

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        
        if !visitor.0.is_empty() {
            // Get current timestamp
            let now = chrono::Local::now();
            let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
            
            match metadata.level().as_str() {
                "ERROR" => println!("[{}] ❌ Error: {} - {}", timestamp, metadata.target(), visitor.0),
                "WARN" => println!("[{}] ⚠️ Warning: {} - {}", timestamp, metadata.target(), visitor.0),
                "INFO" => println!("[{}] ℹ️ {} - {}", timestamp, metadata.target(), visitor.0),
                "DEBUG" => {
                    if metadata.target().contains("service") || metadata.target().contains("generator") {
                        println!("[{}] 🔄 {} - {}", timestamp, metadata.target(), visitor.0);
                    }
                },
                _ => {}
            }
        }
    }
}

pub fn setup() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,sqlx=debug,backend::services=info,backend::generator=info,backend::games=info"));
        
    let subscriber = Registry::default()
        .with(env_filter)
        .with(CustomLayer);
        
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");
} 