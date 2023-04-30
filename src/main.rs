use core::fmt;
use std::{
    collections::{hash_map::Entry, HashMap},
    error::Error,
};

use binrw::{io::Cursor, BinRead};
use btleplug::{
    api::{bleuuid, Central, Manager as _, ScanFilter},
    platform::Manager,
};
use clap::Parser;
use futures::stream::StreamExt;
use rumqttc::{AsyncClient, MqttOptions};
use serde::Serialize;
use tokio::task;

const SERVICE_UUID: uuid::Uuid = bleuuid::uuid_from_u16(0x181a);

#[derive(BinRead, Clone, Copy, Eq, Hash, PartialEq)]
#[br(little)]
#[repr(transparent)]
struct SensorMac([u8; 6]);

impl fmt::Display for SensorMac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[5], self.0[4], self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}

#[derive(BinRead, PartialEq, Serialize)]
#[br(little)]
struct SensorPayload {
    #[serde(skip)]
    mac: SensorMac,
    temperature: i16,
    humidity: u16,
    battery_mv: u16,
    battery_level: u8,
    #[serde(skip)]
    counter: u8,
    #[serde(skip)]
    flags: u8,
}

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    mqtt_url: String,

    #[arg(short, long, default_value_os = "mi_sensor")]
    topic: String,

    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mqtt_opts = MqttOptions::parse_url(args.mqtt_url)?;
    let (client, mut event_loop) = AsyncClient::new(mqtt_opts, 8);

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters.into_iter().nth(0).ok_or("no bluetooth adapter")?;
    let mut events = adapter.events().await?;
    adapter.start_scan(ScanFilter::default()).await?;

    task::spawn(async move {
        while let Ok(notification) = event_loop.poll().await {
            if args.verbose {
                println!("Received = {:?}", notification);
            }
        }
    });

    let mut last_readings: HashMap<SensorMac, SensorPayload> = HashMap::new();

    while let Some(event) = events.next().await {
        match event {
            btleplug::api::CentralEvent::ServiceDataAdvertisement {
                id: _,
                service_data,
            } => {
                if let Some(data) = service_data.get(&SERVICE_UUID) {
                    let mut c = Cursor::new(data);
                    if let Ok(payload) = SensorPayload::read(&mut c) {
                        let payload = match last_readings.entry(payload.mac) {
                            Entry::Vacant(entry) => entry.insert(payload),
                            Entry::Occupied(mut entry) => {
                                if *entry.get() != payload {
                                    entry.insert(payload);
                                    entry.into_mut()
                                } else {
                                    continue;
                                }
                            }
                        };

                        let json_payload = serde_json::to_string(&payload)?;

                        if args.verbose {
                            println!("{}: {}", payload.mac, json_payload);
                        }

                        let topic = format!("{}/{}", args.topic, payload.mac);
                        client
                            .publish(topic, rumqttc::QoS::AtMostOnce, false, json_payload)
                            .await?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}
