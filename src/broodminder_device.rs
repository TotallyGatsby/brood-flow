use std::collections::HashMap;

#[derive(Debug)]
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
    }
  }
}
