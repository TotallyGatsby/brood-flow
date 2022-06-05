use chrono::prelude::{DateTime, Utc};
use rumqttc::AsyncClient;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct BroodminderDevice {
  pub device_id: String,
  pub model: u8,
  pub minor_version: u8,
  pub major_version: u8,
  pub realtime_temp1: u8,
  pub battery_percent: u8,
  pub elapsed1: u8,
  pub elapsed2: u8,
  pub temp1: u8,
  pub temp2: u8,
  pub realtime_temp2: u8,
  pub realtime_temperature_c: f32,
  pub realtime_temperature_f: f32,
  pub temperature_c: f32,
  pub temperature_f: f32,
  last_config_sent: i64,
  last_state_sent: i64,
}

impl BroodminderDevice {
  // Broodminder devices will broadcast 0x028D (653) as their manufacturer specific data id
  pub fn is_broodminder(data: &HashMap<u16, Vec<u8>>) -> bool {
    data.contains_key(&653)
  }

  pub fn build_broodminder_device(data: &Vec<u8>) -> Self {
    Self {
      device_id: "(unknown)".to_string(),
      model: data[0],
      minor_version: data[1],
      major_version: data[2],
      realtime_temp1: data[3],
      battery_percent: data[4],
      elapsed1: data[5],
      elapsed2: data[6],
      temp1: data[7],
      temp2: data[8],
      realtime_temp2: data[9],
      realtime_temperature_c: (256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0,
      realtime_temperature_f: ((256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0) * 9.0
        / 5.0
        + 32.0,
      temperature_c: (256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0,
      temperature_f: ((256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0) * 9.0 / 5.0
        + 32.0,
      last_config_sent: 0,
      last_state_sent: 0,
    }
  }

  pub fn update(&mut self, data: &Vec<u8>) {
    self.realtime_temp1 = data[3];
    self.battery_percent = data[4];
    self.elapsed1 = data[5];
    self.elapsed2 = data[6];
    self.temp1 = data[7];
    self.temp2 = data[8];
    self.realtime_temp2 = data[9];
    self.realtime_temperature_c = (256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0;
    self.realtime_temperature_f =
      ((256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0) * 9.0 / 5.0 + 32.0;
    self.temperature_c = (256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0;
    self.temperature_f =
      ((256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0) * 9.0 / 5.0 + 32.0;
  }

  pub fn send_delete_messages(&self, mut client: AsyncClient) {
    // Home Assistant will delete any device it receives an empty config message for
    // The topic must conform to:
    //   <discovery_prefix>/<component>/[<node_id>/]<object_id>/config
    //   homeassistant/sensor/47:00:00/config
    // A JSON payload must be empty
  }

  pub fn send_state_message(&mut self, mut client: AsyncClient) {
    // State topic should be whatever is set in 'state_topic' in the config message
    // e.g.
    // homeassistant/sensor/47:00:00/state
    // And should contain a json object that can be parsed by the 'value_template'
    // See: https://www.home-assistant.io/docs/configuration/templating/#processing-incoming-data
    if Utc::now().timestamp_millis() - self.last_state_sent > 30000 {
      // No more than 1 per 30s
      info!("Publishing state via MQTT for {:?}", self.device_id);

      self.last_state_sent = Utc::now().timestamp_millis();
    }
  }

  pub fn send_config_messages(&mut self, mut client: AsyncClient) {
    // Home Assistant expects a configuration message be sent for each device:
    // https://www.home-assistant.io/docs/mqtt/discovery/
    // The topic must conform to:
    //   <discovery_prefix>/<component>/[<node_id>/]<object_id>/config
    //   homeassistant/sensor/47:00:00/config
    // A JSON payload must be sent with key/values matching config from here:
    // https://www.home-assistant.io/integrations/sensor.mqtt/
    // Example payload:
    /*
        {
          name: "Hive 1 Temp Sensor",
          device: {
            manufacturer: "Broodminder",
            identifiers: "47:00:00",
          },
          device_class: "temperature",
          expire_after: 3600,
          force_update: true,
          state_class: "measurement",
          unit_of_measurement: "C"
          state_topic: "homeassistant/sensor/47:00:00/state",
          value_template: {{ value_json.temperature_c }}
        }
    */

    // Note that sensors that report more than one value will need to send more than one
    // config message -- one per sensor value type, e.g. humidity and temperature will need
    // different topics

    // Only send config every hour
    if Utc::now().timestamp_millis() - self.last_config_sent > 3600000 {
      // No more than 1 per hour
      info!("Publishing configuration via MQTT for {:?}", self.device_id);

      self.last_config_sent = Utc::now().timestamp_millis();
    }
  }
}
