use std::{
    env,
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread::{self, sleep},
    time::Duration,
};

use nrf24l01::{OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};

use crate::virtual_interface::lib::icmp_reply;

mod virtual_interface;

const DELAY: u64 = 10;

fn main() {
    let args: Vec<String> = env::args().collect();
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

    let mut config = tun::Configuration::default();
    config
        .address((172, 0, 0, tun_addr))
        .netmask((255, 255, 255, 0))
        .destination((172, 0, 0, other_tun_addr))
        .up();

    let dev = tun::create(&config).unwrap();

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

    tun(dev, nrf_rx, nrf_tx);

    // match args[1].as_str() {
    //     "rx" => tun_rx(&mut dev, &mut nrf_rx, &mut nrf_tx),
    //     "tx" => tun_tx(&mut dev, &mut nrf_rx, &mut nrf_tx),
    //     _ => panic!(),
    // }
}

fn tun(dev: tun::platform::Device, mut nrf_rx: NRF24L01, mut nrf_tx: NRF24L01) {
    dev.set_nonblock().unwrap();
    let tun_arc = Arc::new(Mutex::new(dev));

    // RX loop
    let tun = Arc::clone(&tun_arc);
    thread::spawn(move || {
        println!("rx thread started!");
        let mut buf = [0u8; 1024];
        loop {
            let mut offset = 0;
            sleep(Duration::from_millis(1));
            if nrf_rx.data_available().unwrap() {
                let n = nrf_rx
                    .read_all(|packet| {
                        for (i, byte) in packet.iter().enumerate() {
                            buf[i + offset] = *byte;
                        }
                        offset += packet.len();
                    })
                    .unwrap();
                println!("{} packets received", n);
                let mut tun = tun.lock().unwrap();
                tun.write(&buf[0..offset]);
                drop(tun);
            }
        }
    });

    // TX loop
    let tun = Arc::clone(&tun_arc);
    thread::spawn(move || {
        println!("tx thread started!");
        let mut buf = [0u8; 1024];
        loop {
            sleep(Duration::from_millis(1));
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
