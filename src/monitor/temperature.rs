use std::fs;
use std::path::Path;

const SENSOR_IGNORE_LIST: &[&str] = &["PMU tcal", "noname"];

/// Get temperature sensor data from /sys/class/hwmon/
pub fn get_temperatures() -> anyhow::Result<Vec<(String, f64)>> {
    let mut temps = Vec::new();
    let hwmon_path = Path::new("/sys/class/hwmon");

    if !hwmon_path.exists() {
        return Ok(temps);
    }

    for entry in fs::read_dir(hwmon_path)? {
        let entry = entry?;
        let hwmon_dir = entry.path();

        // Get the sensor name
        let name = fs::read_to_string(hwmon_dir.join("name"))
            .unwrap_or_default()
            .trim()
            .to_string();

        // Read all temp*_input files
        for i in 1..=20 {
            let input_path = hwmon_dir.join(format!("temp{}_input", i));
            if !input_path.exists() {
                break;
            }

            if let Ok(content) = fs::read_to_string(&input_path) {
                if let Ok(millidegrees) = content.trim().parse::<i64>() {
                    let temp = millidegrees as f64 / 1000.0;
                    if temp > 0.0 {
                        // Get label if available
                        let label_path = hwmon_dir.join(format!("temp{}_label", i));
                        let sensor_name = if let Ok(label) = fs::read_to_string(&label_path) {
                            label.trim().to_string()
                        } else {
                            format!("{}_{}", name, i)
                        };

                        // Skip ignored sensors
                        if SENSOR_IGNORE_LIST.iter().any(|s| sensor_name.contains(s)) {
                            continue;
                        }

                        temps.push((sensor_name, temp));
                    }
                }
            }
        }
    }

    temps.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(temps)
}
