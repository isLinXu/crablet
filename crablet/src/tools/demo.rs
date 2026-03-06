use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use crate::plugins::Plugin;

pub struct WeatherPlugin;

#[async_trait]
impl Plugin for WeatherPlugin {
    fn name(&self) -> &str {
        "weather"
    }

    fn description(&self) -> &str {
        "Get current weather for a location. Args: { \"location\": \"city name\" }"
    }

    async fn initialize(&mut self) -> Result<()> { Ok(()) }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        // Handle args as map
        let location = if let Some(obj) = args.as_object() {
            obj.get("location").and_then(|v| v.as_str()).unwrap_or("unknown")
        } else {
             args.as_str().unwrap_or("unknown")
        };
        
        if location.to_lowercase().contains("tokyo") {
            Ok("Weather in Tokyo: Sunny, 25°C, Humidity 60%".to_string())
        } else {
            Ok(format!("Weather in {}: Cloudy, 20°C", location))
        }
    }

    async fn shutdown(&mut self) -> Result<()> { Ok(()) }
}

pub struct CalculatorPlugin;

#[async_trait]
impl Plugin for CalculatorPlugin {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Calculate math expression. Args: { \"expression\": \"math string\" }"
    }

    async fn initialize(&mut self) -> Result<()> { Ok(()) }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let expr = if let Some(obj) = args.as_object() {
            obj.get("expression").and_then(|v| v.as_str()).unwrap_or("0")
        } else {
             args.as_str().unwrap_or("0")
        };
        
        match evalexpr::eval(expr) {
            Ok(result) => Ok(result.to_string()),
            Err(e) => Ok(format!("Error: {}", e)),
        }
    }

    async fn shutdown(&mut self) -> Result<()> { Ok(()) }
}
