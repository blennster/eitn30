use std::{
    env,
    io::{Read, Write},
    thread::{self, sleep},
    time::Duration,
};

use nrf24l01::{DataRate, OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};

extern crate tun;

mod virtual_interface;

const MTU: usize = 500;
const BUF_SIZE: usize = 4096;
const RESET_BUF: [u8; 5] = [255, 255, 255, 255, 255];

macro_rules! debug_println {
    ($($rest:tt)*) => {
        if std::env::var("DEBUG").is_ok() {
            std::println!($($rest)*);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let addr = match args[1].as_str() {
        "rx" => *b"abcde",
        "tx" => *b"edcba",
        _ => panic!(),
    };
    let other_addr = match args[1].as_str() {
        "rx" => *b"edcba",
        "tx" => *b"abcde",
        _ => panic!(),
    };
    let tun_addr = match args[1].as_str() {
        "tx" => 1,
        "rx" => 2,
        _ => panic!(),
    };
    let other_tun_addr = match args[1].as_str() {
        "tx" => 2,
        "rx" => 1,
        _ => panic!(),
    };

    let delay = args[2].parse::<u64>().unwrap();

    let mut config = tun::Configuration::default();
    config
        .address((172, 0, 0, tun_addr))
        .netmask((255, 255, 255, 0))
        .destination((172, 0, 0, other_tun_addr))
        .mtu(MTU as i32)
        .up();

    let dev = tun::create(&config).unwrap();

    let config_rx = RXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: addr,
        data_rate: DataRate::R2Mbps,
        ..Default::default()
    };
    let config_tx = TXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: other_addr,
        max_retries: 7,
        retry_delay: 1,
        data_rate: DataRate::R2Mbps,
    };

    assert!(
        config_rx.data_rate == DataRate::R2Mbps
            && config_rx.data_rate == config_tx.data_rate
            && config_rx.channel == config_tx.channel
            && config_rx.pipe0_address != config_tx.pipe0_address
    );

    let mut nrf_rx = NRF24L01::new(17, 0, 0).unwrap();
    nrf_rx.configure(&OperatingMode::RX(config_rx)).unwrap();
    nrf_rx.flush_input().unwrap();
    nrf_rx.listen().unwrap();

    let mut nrf_tx = NRF24L01::new(27, 1, 0).unwrap();
    nrf_tx.configure(&OperatingMode::TX(config_tx)).unwrap();
    nrf_tx.flush_output().unwrap();

    tun(dev, nrf_rx, nrf_tx, delay);
}

fn tun(dev: tun::platform::Device, mut nrf_rx: NRF24L01, mut nrf_tx: NRF24L01, delay: u64) {
    let (mut reader, mut writer) = dev.split();

    // RX loop
    let rx = thread::spawn(move || {
        println!("rx thread started!");
        let mut buf = [0u8; 4096];
        let mut end = 0;
        let mut reset_buf = false;

        loop {
            sleep(Duration::from_micros(delay));
            if nrf_rx.data_available().unwrap() {
                if end + 96 >= BUF_SIZE {
                    end = 0;
                }

                let n = nrf_rx
                    // read all cannot have range checks since it blocks the
                    // radio hardware hurting performance
                    .read_all(|packet| {
                        for byte in packet.iter() {
                            buf[end] = *byte;
                            end += 1;
                        }
                    })
                    .unwrap() as u32;

                for packet in buf[0..end].chunks(32) {
                    if packet.eq(&RESET_BUF) {
                        reset_buf = true;
                        break;
                    }
                }

                if reset_buf {
                    end = 0;
                    reset_buf = false;
                    debug_println!("reset buf command received");
                    continue;
                }

                debug_println!("{} packets received", n);
                // Make sure the packet is valid
                let pkt = &buf[0..end];
                if end != 0 && packet::ip::Packet::new(pkt).is_ok() {
                    let result = writer.write(pkt);
                    match result {
                        Ok(n) => debug_println!("{} bytes written to interface", n),
                        Err(err) => debug_println!("{} error when writing to interface", err),
                    }
                    end = 0;
                }
            }
        }
    });

    // TX loop
    let tx = thread::spawn(move || {
        println!("tx thread started!");
        let mut buf = [0u8; BUF_SIZE];
        loop {
            sleep(Duration::from_micros(delay));
            let read_result = reader.read(&mut buf);
            if let Ok(n) = read_result {
                if n == 0 {
                    continue;
                }

                let pkt = &buf[0..n];
                if packet::ip::Packet::new(pkt).is_err() {
                    debug_println!("read an invalid packet from tun, dropping");
                    continue;
                }

                let mut chunks = vec![];
                let mut queue = vec![];

                for chunk in pkt.chunks(32) {
                    queue.push(chunk);
                    if queue.len() == 2 {
                        chunks.push(queue.clone());
                        queue.clear();
                    }
                }
                if !queue.is_empty() {
                    chunks.push(queue);
                }

                for queue in chunks {
                    for pkt in queue {
                        nrf_tx.push(0, pkt).unwrap();
                        debug_println!("queuing {:?}", pkt);
                    }
                    match nrf_tx.send() {
                        Ok(retries) => debug_println!("message sent, {} retries needed", retries),
                        Err(err) => {
                            debug_println!("destination unreachable: {:?}", err);
                            nrf_tx.flush_output().unwrap();
                            // The packet cannot be sent in any meaningful way, abort
                            break;
                        }
                    };
                    sleep(Duration::from_micros(delay / 2));
                }

                // Send reset buf command, may fail but not really important
                nrf_tx.push(0, &RESET_BUF).ok();
                nrf_tx.send().ok();
                nrf_tx.flush_output().unwrap();
            }
        }
    });

    rx.join().unwrap();
    tx.join().unwrap();
}
