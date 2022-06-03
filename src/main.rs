#[macro_use]
extern crate log;
extern crate env_logger;

use std::collections::HashMap;
use std::error::Error;

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use config::Config;
use futures::stream::StreamExt;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Configuration {
  devices: Vec<DeviceConfiguration>,
}

#[derive(Debug, Deserialize)]
struct DeviceConfiguration {
  id: Option<String>,     // The Broodminder issued ID, eg "47:01:01"
  name: Option<String>,   // A name for the device for your reference
  topic: Option<String>,  // The MQTT topic to publish updates to
  realtime: Option<bool>, // If true, publishes realtime temperature data. If false reports broodminder aggregated temp information
}

#[derive(Debug)]
struct BroodminderDevice {
  model: u8,
  minor_version: u8,
  major_version: u8,
  realtime_temp1: u8,
  battery_percent: u8,
  elapsed1: u8,
  elapsed2: u8,
  temp1: u8,
  temp2: u8,
  realtime_temp2: u8,
  realtime_temperature_c: f32,
  realtime_temperature_f: f32,
  temperature_c: f32,
  temperature_f: f32,
}

async fn get_central(manager: &Manager) -> Adapter {
  let adapters = manager.adapters().await.unwrap();
  adapters.into_iter().nth(0).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  env_logger::init_from_env(
    env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
  );

  // TODO: Handle when the configuration file isn't found
  let settings = Config::builder()
    .add_source(config::File::with_name("configuration.yml"))
    .build()
    .unwrap();

  info!(
    "Settings: {:?}",
    settings.try_deserialize::<Configuration>().unwrap()
  );

  let manager = Manager::new().await?;

  // get the first bluetooth adapter
  // connect to the adapter
  let central = get_central(&manager).await;

  // Each adapter has an event stream, we fetch via events(),
  // simplifying the type, this will return what is essentially a
  // Future<Result<Stream<Item=CentralEvent>>>.
  let mut events = central.events().await?;

  // start scanning for devices
  central.start_scan(ScanFilter::default()).await?;

  // Print based on whatever the event receiver outputs.
  // This should be run in its own thread (or task? once btleplug uses async channels).
  while let Some(event) = events.next().await {
    if let CentralEvent::ManufacturerDataAdvertisement {
      id,
      manufacturer_data,
    } = event
    {
      if is_broodminder(&manufacturer_data) {
        let brood_data = get_broodminder_data(&manufacturer_data[&653]);
        let peripheral = central.peripheral(&id).await?;
        let properties = peripheral.properties().await?;

        info!(
          "{:?} - Peripheral {:?}",
          brood_data,
          properties
            .unwrap()
            .local_name
            .unwrap_or(String::from("00:00:00"))
        );
      }
    }
  }

  Ok(())
}

// Broodminder devices will broadcast 0x028D (653) as their manufacturer specific data id
fn is_broodminder(data: &HashMap<u16, Vec<u8>>) -> bool {
  data.contains_key(&653)
}

fn get_broodminder_data(data: &Vec<u8>) -> BroodminderDevice {
  BroodminderDevice {
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
    temperature_f: ((256.0 * data[8] as f32 + data[7] as f32 - 5000.0) / 100.0) * 9.0 / 5.0 + 32.0,
  }
}
