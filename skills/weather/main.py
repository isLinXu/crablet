#!/usr/bin/env python3
"""
Weather Skill - Get weather information for any location
"""

import json
import sys
import os


def get_weather(location: str, units: str = "metric") -> dict:
    """
    Get weather information for a location.
    
    In a real implementation, this would call a weather API like OpenWeatherMap.
    For demonstration, we return mock data.
    """
    # Mock weather data
    conditions = ["Sunny", "Cloudy", "Rainy", "Partly Cloudy", "Clear"]
    
    # Use location hash to get consistent results for the same location
    location_hash = sum(ord(c) for c in location)
    temp_base = 15 + (location_hash % 15)  # 15-30°C
    
    if units == "imperial":
        temp = int(temp_base * 9/5 + 32)
        unit_symbol = "°F"
    else:
        temp = temp_base
        unit_symbol = "°C"
    
    condition = conditions[location_hash % len(conditions)]
    humidity = 40 + (location_hash % 40)  # 40-80%
    
    return {
        "location": location,
        "temperature": temp,
        "unit": unit_symbol,
        "condition": condition,
        "humidity": f"{humidity}%",
        "wind_speed": f"{5 + (location_hash % 15)} km/h",
        "source": "Mock Weather API (Demo)"
    }


def main():
    try:
        # Parse arguments
        if len(sys.argv) < 2:
            print(json.dumps({
                "error": "No arguments provided",
                "usage": "python main.py '{\"location\": \"Beijing\", \"units\": \"metric\"}'"
            }), file=sys.stderr)
            sys.exit(1)
        
        args = json.loads(sys.argv[1])
        location = args.get("location")
        units = args.get("units", "metric")
        
        if not location:
            print(json.dumps({
                "error": "Missing required parameter: location"
            }), file=sys.stderr)
            sys.exit(1)
        
        # Get weather data
        weather = get_weather(location, units)
        
        # Output result
        print(json.dumps(weather, ensure_ascii=False, indent=2))
        
    except json.JSONDecodeError as e:
        print(json.dumps({
            "error": f"Invalid JSON arguments: {str(e)}"
        }), file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(json.dumps({
            "error": f"Unexpected error: {str(e)}"
        }), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
