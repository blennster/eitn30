use std::env;
use std::thread::sleep;
use std::time::Duration;

use nrf24l01::{OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};

fn main() {
    let args = env::args().collect::<String>();

    if args.contains("rx") {
        let config = RXConfig {
            channel: 0,
            pa_level: PALevel::Low,
            pipe0_address: *b"abcde",
            ..Default::default()
        };
        let mut device = NRF24L01::new(27, 1).unwrap();
        device.configure(&OperatingMode::RX(config)).unwrap();
        device.listen().unwrap();
        loop {
            sleep(Duration::from_millis(500));
            if device.data_available().unwrap() {
                device
                    .read_all(|packet| {
                        println!("Received {:?} bytes", packet.len());
                        println!("Payload {:?}", packet);
                    })
                    .unwrap();
            }
        }
    } else if args.contains("tx") {
        let config = TXConfig {
            channel: 0,
            pa_level: PALevel::Low,
            pipe0_address: *b"abcde",
            max_retries: 3,
            retry_delay: 2,
            ..Default::default()
        };
        let mut device = NRF24L01::new(17, 0).unwrap();
        let message = b"sendtest";
        device.configure(&OperatingMode::TX(config)).unwrap();
        device.flush_output().unwrap();
        loop {
            device.push(0, message).unwrap();
            match device.send() {
                Ok(retries) => println!("Message sent, {} retries needed", retries),
                Err(err) => {
                    println!("Destination unreachable: {:?}", err);
                    device.flush_output().unwrap()
                }
            };
            sleep(Duration::from_millis(5000));
        }
    } else {
        println!("specify tx or rx");
    }
}
