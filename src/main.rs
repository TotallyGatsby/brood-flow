#[macro_use]
extern crate log;

mod brood_flow_config;
mod broodminder_device;

use brood_flow_config::Configuration;
use broodminder_device::BroodminderDevice;

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use config::Config;
use futures::stream::StreamExt;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

async fn get_central(manager: &Manager) -> Adapter {
  let adapters = manager.adapters().await.unwrap();
  adapters.into_iter().nth(0).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Initialize logging at log level info by default
  env_logger::init_from_env(
    env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
  );

  // Load configuration.yaml into our Configuration object
  // TODO: Handle when the configuration file isn't found
  let settings = Config::builder()
    .add_source(config::File::with_name("configuration.yml"))
    .build()
    .unwrap()
    .try_deserialize::<Configuration>()
    .unwrap();

  info!("Settings: {:?}", settings);

  // Set up the MQTT connection
  // TODO: Be resilient to MQTT disconnections
  let mut mqttoptions = MqttOptions::new(
    "brood-flow",
    settings.broker_host.unwrap(),
    settings.broker_port.unwrap(),
  );
  mqttoptions.set_keep_alive(Duration::from_secs(5));

  // Not sure why, but the client doesn't send if it's not marked as mutable here
  let (mut client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

  /*
  tokio::task::spawn(async move {
    for i in 0..10 {
      match client
        .publish("hello/rumqtt", QoS::AtLeastOnce, false, vec![i; i as usize])
        .await
      {
        Err(error) => info!("Error: {:?}", error),
        Ok(_) => info!("Sent!"),
      }

      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  });*/

  // get the first bluetooth adapter
  // connect to the adapter
  let btle_manager = Manager::new().await?;
  let central = get_central(&btle_manager).await;

  // Each adapter has an event stream, we fetch via events(),
  // This will return what is essentially a
  // Future<Result<Stream<Item=CentralEvent>>>.
  let mut events = central.events().await?;

  // Start scanning for BTLE devices
  // TODO: Add a scan filter?
  central.start_scan(ScanFilter::default()).await?;

  // Cache of discovered devices
  let mut devices: HashMap<String, BroodminderDevice> = HashMap::new();

  info!("Listening for Broodminder events");
  // When events are received by the BTLE stream, process them
  // TODO: This should be run in its own thread (or task(?) once btleplug uses async channels).
  while let Some(event) = events.next().await {
    // Right now, we only care about the data advertisements from the Broodminder devices
    if let CentralEvent::ManufacturerDataAdvertisement {
      id,
      manufacturer_data,
    } = event
    {
      // Ensure we're only reading data from Broodminder devices
      if BroodminderDevice::is_broodminder(&manufacturer_data) {
        let peripheral = central.peripheral(&id).await?;
        let properties = peripheral.properties().await?;
        let device_id = properties
          .unwrap()
          .local_name
          .unwrap_or(String::from("00:00:00"));

        if devices.contains_key(&device_id) {
          // Update the previous object
          devices
            .entry(device_id.clone())
            .or_default()
            .update(&manufacturer_data[&653]);
          info!("Updated Device: {:?}", devices[&device_id]);
        } else {
          // Instantiate an object
          let mut brood_data =
            BroodminderDevice::build_broodminder_device(&manufacturer_data[&653]);
          brood_data.device_id = device_id.clone();

          info!("New Broodminder device detected: {:?}", brood_data);

          devices.insert(device_id.clone(), brood_data);
        }

        // Send MQTT messages
        devices
          .entry(device_id.clone())
          .and_modify(|device| device.send_config_messages(client.clone()));
        devices
          .entry(device_id.clone())
          .and_modify(|device| device.send_state_message(client.clone()));
      }
    }
  }

  Ok(())
}
