#[macro_use]
extern crate log;

mod brood_flow_config;
mod broodminder_device;

use broodminder_device::BroodminderDevice;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use futures::stream::StreamExt;
use rumqttc::{AsyncClient, MqttOptions};
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
  let settings = brood_flow_config::get_config().unwrap();
  info!("Settings: {:?}", settings);

  // Set up the MQTT connection
  // TODO: Be resilient to MQTT disconnections?
  let mut mqttoptions = MqttOptions::new(
    "brood-flow",
    settings.broker_host.unwrap(),
    settings.broker_port.unwrap(),
  );
  mqttoptions.set_keep_alive(Duration::from_secs(5));

  // Not sure why, but the client doesn't send if it's not marked as mutable here
  #[allow(unused_mut)]
  let (mut client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

  // Get the first bluetooth adapter and connect to the adapter
  let btle_manager = Manager::new().await?;
  let central = get_central(&btle_manager).await;

  // Each adapter has an event stream, we fetch via events(),
  // This will return what is essentially:
  // Future<Result<Stream<Item=CentralEvent>>>.
  let mut events = central.events().await?;

  // Start scanning for BTLE devices
  // TODO: Add a scan filter?
  central.start_scan(ScanFilter::default()).await?;

  // Cache of discovered devices, as we want to store when the last message was sent per device
  let mut devices: HashMap<String, BroodminderDevice> = HashMap::new();

  // Start a task to listen for BTLE events
  tokio::task::spawn(async move {
    info!("Listening for Broodminder events.");
    // When events are received by the BTLE stream, process them
    while let Some(event) = events.next().await {
      // Right now, we only care about the Data Advertisements from the Broodminder devices
      if let CentralEvent::ManufacturerDataAdvertisement {
        id,
        manufacturer_data,
      } = event
      {
        // Ensure we're only reading data from Broodminder devices
        if BroodminderDevice::is_broodminder(&manufacturer_data) {
          let peripheral = central.peripheral(&id).await.unwrap();
          let properties = peripheral.properties().await.unwrap();
          let device_id = properties
            .unwrap()
            .local_name
            .unwrap_or(String::from("00:00:00")); // Sometimes device ID doesn't correctly populate

          if devices.contains_key(&device_id) {
            // Update the previous object if we've already seen it
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

          // Send our config and state messages (these functions already handle rate limiting)
          if settings.mqtt_enabled {
            devices
              .entry(device_id.clone())
              .and_modify(|device| device.send_config_messages(client.clone()));

            devices
              .entry(device_id.clone())
              .and_modify(|device| device.send_state_message(client.clone()));
          }
        }
      }
    }
  });

  // Pump the MQTT eventloop
  loop {
    let event = eventloop.poll().await;
    match event {
      Ok(rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(msg))) => {
        info!("Connected to the broker!");
        debug!("Connected msg = {msg:?}");
      }
      Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => {
        warn!("Disconnected, retry happening...");
      }
      Ok(msg) => {
        debug!("Event = {msg:?}");
      }
      Err(e) => {
        error!("Error = {}", e);
        error!("Terminating...");
        break;
      }
    }
  }
  Ok(())
}
