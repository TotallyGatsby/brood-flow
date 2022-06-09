use chrono::prelude::Utc;
use json::object;
use rumqttc::{AsyncClient, QoS};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct BroodminderDevice {
  pub device_id: String,
  pub model: u8,
  pub minor_version: u8,
  pub major_version: u8,
  pub realtime_temp1: u8, // Realtime temperature can update every advertisement and is not aggregated
  pub battery_percent: u8,
  pub elapsed1: u8, // How many 'ticks' have passed, I believe this is an internal aggregation/smoothing mechanism
  pub elapsed2: u8,
  pub temp1: u8, // Temperature updates every 'elapsed' tick, and is an aggregated value
  pub temp2: u8,
  pub realtime_temp2: u8, // Realtime temp uses two bytes and some math to calculate
  pub realtime_weight1: u8, // Two bytes representing the total weight of the scale
  pub realtime_weight2: u8,
  // Broodminder devices also report left and right weight independently, but that seems
  // like overkill for this application. If someone has a need, it wouldn't be difficult to add

  // Calculated sensor values
  pub realtime_temperature_c: f32,
  pub realtime_temperature_f: f32,
  pub temperature_c: f32,
  pub temperature_f: f32,
  pub realtime_weight_kg: f32,
  pub realtime_weight_lbs: f32, // Home Assistant doesn't autotranslate kgs to lbs for some reason?

  // Millisecond epoch time since last messages were sent for this device, for rate limiting
  last_config_sent: i64,
  last_state_sent: i64,
}

// TODO: Cleanup logging, use a consistent approach to what should and shouldn't be logged

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
      realtime_weight1: data[19],
      realtime_weight2: data[20],
      // Pulled from the Broodminder manual
      realtime_temperature_c: (256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0,
      realtime_temperature_f: ((256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0) * 9.0
        / 5.0
        + 32.0,
      temperature_c: (256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0,
      temperature_f: ((256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0) * 9.0 / 5.0
        + 32.0,

      realtime_weight_kg: ((256.0 * data[20] as f32) - data[19] as f32 - 32767.0) as f32 / 100.0,
      realtime_weight_lbs: 2.204623
        * ((256.0 * data[20] as f32) - data[19] as f32 - 32767.0) as f32
        / 100.0,
      last_config_sent: 0,
      last_state_sent: 0,
    }
  }

  // Take in a Data Advertisement and parse it into fields, updating in place
  pub fn update(&mut self, data: &Vec<u8>) {
    self.realtime_temp1 = data[3];
    self.battery_percent = data[4];
    self.elapsed1 = data[5];
    self.elapsed2 = data[6];
    self.temp1 = data[7];
    self.temp2 = data[8];
    self.realtime_temp2 = data[9];
    self.realtime_weight1 = data[19];
    self.realtime_weight2 = data[20];

    self.realtime_temperature_c = (256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0;
    self.realtime_temperature_f =
      ((256.0 * data[9] as f32 + data[3] as f32 - 5000.0) / 100.0) * 9.0 / 5.0 + 32.0;
    self.temperature_c = (256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0;
    self.temperature_f =
      ((256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0) * 9.0 / 5.0 + 32.0;
    self.realtime_weight_kg =
      ((256.0 * data[20] as f32) - data[19] as f32 - 32767.0) as f32 / 100.0;
    self.realtime_weight_lbs =
      2.204623 * ((256.0 * data[20] as f32) - data[19] as f32 - 32767.0) as f32 / 100.0;
  }

  // Keeping this method here for now as documentation for how to send messages that remove devices from
  // HomeAssistant, should that become necessary in the future.
  #[allow(unused_mut, dead_code)] // Client needs to be mutable to send messages for some reason
  pub fn send_delete_messages(&self, mut _client: AsyncClient) {
    // Home Assistant will delete any device it receives an empty config message for
    // The topic must conform to:
    //   <discovery_prefix>/<component>/[<node_id>/]<object_id>/config
    //   homeassistant/sensor/47:00:00/config
    // A JSON payload must be empty
  }

  // Sends a HomeAssistant compatible MQTT message with an update on the state of the device
  // (e.g. the current temperature, humidity, weight, or other data as appropriate)
  #[allow(unused_mut)] // Client needs to be mutable to send messages for some reason
  pub fn send_state_message(&mut self, mut client: AsyncClient) {
    if self.device_id == "00:00:00" {
      return ();
    }

    // State topic should be whatever is set in 'state_topic' in the config message
    // e.g.
    // homeassistant/sensor/47:00:00/state
    // And should contain a json object that can be parsed by the 'value_template'
    // See: https://www.home-assistant.io/docs/configuration/templating/#processing-incoming-data
    if Utc::now().timestamp_millis() - self.last_state_sent > 30000 {
      // No more than 1 per 30s
      // TODO: Magic numbers should be managed by config
      info!("Publishing state via MQTT for {:?}", self.device_id);

      let simple_id = self.device_id.clone().replace(":", "");

      let mut state_message = object! {
        temperature_c: self.realtime_temperature_c,
      };

      // Scales (model number 57) emit a weight value as well
      if self.model == 57 {
        state_message["weight_lbs"] = self.realtime_weight_lbs.into();
      }

      let state_topic = format!("homeassistant/sensor/BM{}/state", simple_id);
      info!("Publishing: {} to {}", state_message.dump(), state_topic);

      tokio::task::spawn(async move {
        match client
          .publish(state_topic, QoS::AtLeastOnce, false, state_message.dump())
          .await
        {
          Err(error) => info!("Error: {:?}", error),
          Ok(_) => info!("Sent state!"),
        }
      });

      self.last_state_sent = Utc::now().timestamp_millis();
    }
  }

  #[allow(unused_mut)] // Client needs to be mutable to send messages for some reason
  pub fn send_config_messages(&mut self, mut client: AsyncClient) {
    if self.device_id == "00:00:00" {
      return ();
    }
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

    // TODO: Magic numbers should probably be config managed
    // Only send config every hour
    if Utc::now().timestamp_millis() - self.last_config_sent > 3600000 {
      let simple_id = self.device_id.clone().replace(":", "");

      // No more than 1 per hour
      info!("Publishing configuration via MQTT for {:?}", self.device_id);

      // Send temperature configuration message
      if self.model == 47 || self.model == 57 {
        let config_message = object! {
          name: format!("{}_temperature", &self.device_id),
          device_class: "temperature",
          expire_after: 3600,
          force_update: true,
          state_class: "measurement",
          unit_of_measurement: "°C",
          state_topic: format!("homeassistant/sensor/BM{}/state", simple_id),
          value_template: "{{ value_json.temperature_c }}",
          unique_id: format!("{}_temperature", simple_id),
        };

        let config_topic = format!("homeassistant/sensor/BM{}Temp/config", simple_id);
        info!("Config message: {:?}", config_message.dump());
        let task_client = client.clone();
        tokio::task::spawn(async move {
          match task_client
            .publish(config_topic, QoS::AtLeastOnce, false, config_message.dump())
            .await
          {
            Err(error) => info!("Error: {:?}", error),
            Ok(_) => info!("Sent config!"),
          }
        });
      }

      // Send weight configuration message
      if self.model == 57 {
        let config_message = object! {
          name: format!("{}_weight", &self.device_id),
          expire_after: 3600,
          force_update: true,
          state_class: "measurement",
          unit_of_measurement: "kg",
          state_topic: format!("homeassistant/sensor/BM{}/state", simple_id),
          value_template: "{{ value_json.weight_lbs }}",
          unique_id: format!("{}_weight", simple_id),
        };

        let config_topic = format!("homeassistant/sensor/BM{}Weight/config", simple_id);
        info!("Config message: {:?}", config_message.dump());
        let task_client = client.clone();
        tokio::task::spawn(async move {
          match task_client
            .publish(config_topic, QoS::AtLeastOnce, false, config_message.dump())
            .await
          {
            Err(error) => info!("Error: {:?}", error),
            Ok(_) => info!("Sent config!"),
          }
        });
      }

      self.last_config_sent = Utc::now().timestamp_millis();
    }
  }
}
