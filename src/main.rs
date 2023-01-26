use std::{
    env,
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread::{self, sleep},
    time::Duration,
};

use nrf24l01::{DataRate, OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};

extern crate tun;

mod virtual_interface;

const MTU: usize = 200;

macro_rules! println {
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

    let config = RXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: addr,
        data_rate: DataRate::R2Mbps,
        ..Default::default()
    };
    let mut nrf_rx = NRF24L01::new(17, 0, 0).unwrap();
    nrf_rx.configure(&OperatingMode::RX(config)).unwrap();
    nrf_rx.listen().unwrap();
    let config = TXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: other_addr,
        max_retries: 7,
        retry_delay: 2,
        data_rate: DataRate::R2Mbps,
    };
    let mut nrf_tx = NRF24L01::new(27, 1, 0).unwrap();
    nrf_tx.configure(&OperatingMode::TX(config)).unwrap();
    nrf_tx.flush_output().unwrap();

    tun(dev, nrf_rx, nrf_tx, delay);

    // match args[1].as_str() {
    //     "rx" => tun_rx(&mut dev, &mut nrf_rx, &mut nrf_tx),
    //     "tx" => tun_tx(&mut dev, &mut nrf_rx, &mut nrf_tx),
    //     _ => panic!(),
    // }
}

fn tun(mut dev: tun::platform::Device, mut nrf_rx: NRF24L01, mut nrf_tx: NRF24L01, delay: u64) {
    dev.set_nonblock().unwrap();
    let tun_arc = Arc::new(Mutex::new(dev));

    // RX loop
    let tun = Arc::clone(&tun_arc);
    thread::spawn(move || {
        println!("rx thread started!");
        let mut buf = [0u8; MTU];
        let mut offset = 0;
        loop {
            sleep(Duration::from_micros(delay));
            if nrf_rx.data_available().unwrap() {
                let n = nrf_rx
                    .read_all(|packet| {
                        for (i, byte) in packet.iter().enumerate() {
                            if offset + i >= MTU {
                                offset = 0;
                                break;
                            }
                            buf[i + offset] = *byte;
                        }
                        offset += packet.len();
                    })
                    .unwrap();
                if offset >= MTU {
                    offset = 0;
                    println!("buf is bigger than mtu, resetting");
                    continue;
                }

                println!("{} packets received", n);
                // Make sure the packet is valid
                if packet::ip::Packet::new(&buf[0..offset]).is_ok() {
                    let mut tun = tun.lock().unwrap();
                    let result = tun.write(&buf[0..offset]);
                    drop(tun);
                    match result {
                        Ok(n) => println!("{} bytes written to interface", n),
                        Err(_) => println!("could not write to interface"),
                    }
                    offset = 0;
                }
            }
        }
    });

    // TX loop
    let tun = Arc::clone(&tun_arc);
    thread::spawn(move || {
        println!("tx thread started!");
        let mut buf = [0u8; MTU];
        loop {
            sleep(Duration::from_micros(delay));
            let mut tun = tun.lock().unwrap();
            let read_result = tun.read(&mut buf);
            drop(tun);
            if let Ok(n) = read_result {
                if n == 0 {
                    continue;
                }

                let pkt = &buf[0..n];
                let mut chunks = vec![];
                let mut queue = vec![];
                for (i, chunk) in pkt.chunks(32).enumerate() {
                    queue.push(chunk);
                    if (i + 1) % 3 == 0 {
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
                    }
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
    })
    .join()
    .unwrap();
}
