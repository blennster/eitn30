use std::{
    env,
    io::{Read, Write},
    thread::sleep,
    time::Duration,
};

use nrf24l01::{OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};

use crate::virtual_interface::lib::icmp_reply;

mod virtual_interface;

const DELAY: u64 = 10;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut config = tun::Configuration::default();
    let addr = match args[1].as_str() {
        "rx" => *b"abcde",
        "tx" => *b"edcba",
        _ => panic!(),
    };
    let other_addr = match args[1].as_str() {
        "tx" => *b"abcde",
        "rx" => *b"edcba",
        _ => panic!(),
    };

    config
        .address((172, 0, 0, 1))
        .netmask((255, 255, 255, 128))
        .up();

    let mut dev = tun::create(&config).unwrap();
    let config = RXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: addr,
        ..Default::default()
    };
    let mut nrf_rx = NRF24L01::new(17, 0, 0).unwrap();
    nrf_rx.configure(&OperatingMode::RX(config)).unwrap();
    nrf_rx.listen().unwrap();
    let config = TXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: other_addr,
        max_retries: 3,
        retry_delay: 2,
        ..Default::default()
    };
    let mut nrf_tx = NRF24L01::new(27, 1, 0).unwrap();
    nrf_tx.configure(&OperatingMode::TX(config)).unwrap();
    nrf_tx.flush_output().unwrap();

    match args[1].as_str() {
        "rx" => tun_rx(&mut dev, &mut nrf_rx, &mut nrf_tx),
        "tx" => tun_tx(&mut dev, &mut nrf_rx, &mut nrf_tx),
        _ => panic!(),
    }
}

fn tun_tx(dev: &mut tun::platform::Device, nrf_rx: &mut NRF24L01, nrf_tx: &mut NRF24L01) {
    loop {
        let mut buf = [0; 1024];
        let mut offset = 0;
        let n = dev.read(&mut buf).unwrap();
        let pkt = &buf[0..n];
        println!("pkt size: {}", n);
        for chunk in pkt.chunks(32) {
            nrf_tx.push(0, chunk).unwrap();
        }
        match nrf_tx.send() {
            Ok(retries) => println!("Message sent, {} retries needed", retries),
            Err(err) => {
                println!("Destination unreachable: {:?}", err);
                nrf_tx.flush_output().unwrap()
            }
        };
        sleep(Duration::from_millis(DELAY));
        if nrf_rx.data_available().unwrap() {
            nrf_rx
                .read_all(|packet| {
                    for (i, byte) in packet.iter().enumerate() {
                        buf[i + offset] = *byte;
                    }
                    offset += packet.len();
                })
                .unwrap();
            dev.write(&buf);
        }
        sleep(Duration::from_millis(DELAY));
    }
}

fn tun_rx(dev: &mut tun::platform::Device, nrf_rx: &mut NRF24L01, nrf_tx: &mut NRF24L01) {
    loop {
        let mut buf = [0u8; 1024];
        let mut offset = 0;
        sleep(Duration::from_millis(DELAY));
        if nrf_rx.data_available().unwrap() {
            println!("received packet!");
            nrf_rx
                .read_all(|packet| {
                    for (i, byte) in packet.iter().enumerate() {
                        buf[i + offset] = *byte;
                    }
                    offset += packet.len();
                })
                .unwrap();
            if let Ok(pkt) = icmp_reply(&buf) {
                for chunk in pkt.chunks(32) {
                    nrf_tx.push(0, chunk).unwrap();
                }
                match nrf_tx.send() {
                    Ok(retries) => println!("Message sent, {} retries needed", retries),
                    Err(err) => {
                        println!("Destination unreachable: {:?}", err);
                        nrf_tx.flush_output().unwrap()
                    }
                };
            } else {
                nrf_tx.push(0, b"packet fail").unwrap();
                match nrf_tx.send() {
                    Ok(retries) => println!("Message sent, {} retries needed", retries),
                    Err(err) => {
                        println!("Destination unreachable: {:?}", err);
                        nrf_tx.flush_output().unwrap()
                    }
                };
            }
        }
    }
}
