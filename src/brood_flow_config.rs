use config::{Config, ConfigError};
use serde::Deserialize;

// WARNING: The configuration.yaml file is not stable yet

#[derive(Debug, Deserialize)]
pub struct Configuration {
  pub devices: Vec<DeviceConfiguration>,
  pub broker_host: Option<String>, // The hostname/IP of the MQTT broker
  pub broker_port: Option<u16>,    // The port for the MQTT broker
  pub mqtt_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeviceConfiguration {
  pub id: Option<String>,     // The Broodminder issued ID, eg "47:01:01"
  pub name: Option<String>,   // A name for the device for your reference
  pub topic: Option<String>,  // The MQTT topic to publish updates to
  pub realtime: Option<bool>, // If true, publishes realtime temperature data. If false reports broodminder aggregated temp information
}

// TODO: Better error handling is probably a good idea here
pub fn get_config() -> Result<Configuration, ConfigError> {
  Ok(
    Config::builder()
      .set_default("mqtt_enabled", true)?
      .add_source(config::File::with_name("configuration.yml"))
      .build()
      .unwrap()
      .try_deserialize::<Configuration>()
      .unwrap(),
  )
}
