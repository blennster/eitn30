use std::env;
use std::thread::sleep;
use std::time::Duration;

use nrf24l01::{OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);
    let t = args[1].clone();
    let spi = args[2].parse::<u8>().unwrap();
    let ce = match spi {
        0 => 17,
        1 => 27,
        _ => panic!(),
    };

    if t == "rx" {
        let config = RXConfig {
            channel: 0,
            pa_level: PALevel::Low,
            pipe0_address: *b"abcde",
            ..Default::default()
        };
        let mut device = NRF24L01::new(ce, spi, 0).unwrap();
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
    } else if t == "tx" {
        let config = TXConfig {
            channel: 0,
            pa_level: PALevel::Low,
            pipe0_address: *b"abcde",
            max_retries: 3,
            retry_delay: 2,
            ..Default::default()
        };
        let mut device = NRF24L01::new(ce, spi, 0).unwrap();
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
