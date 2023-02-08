use std::{
    env,
    io::{Read, Write},
    thread::{self, sleep},
    time::Duration,
};

use nrf24l01::{DataRate, OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};
use tun::platform::posix::{Reader, Writer};

extern crate tun;

const MTU: usize = 900;
const BUF_SIZE: usize = 4096;

macro_rules! debug_println {
    ($($rest:tt)*) => {
        if std::env::var("DEBUG").is_ok() {
            std::println!($($rest)*);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut rx_addr = *b"rx0";

    let input_addr = args[1]
        .parse::<u8>()
        .expect("addr needs to be number in range [0,255]");

    if input_addr == 0 || input_addr == 255 {
        panic!("address 0 or 255 is not allowed");
    }

    *rx_addr.last_mut().unwrap() = input_addr;

    let delay = args[2]
        .parse::<u64>()
        .expect("loop delay in micros must be provided");

    let mut config = tun::Configuration::default();
    config
        .address((172, 0, 0, input_addr))
        .netmask((255, 255, 255, 0))
        .mtu(MTU as i32)
        .up();

    let dev = tun::create(&config).unwrap();

    let config_rx = RXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: rx_addr,
        data_rate: DataRate::R2Mbps,
        ..Default::default()
    };
    let mut tx_addr = rx_addr;
    *tx_addr.last_mut().unwrap() += 1;
    let config_tx = TXConfig {
        channel: 110,
        pa_level: PALevel::Low,
        pipe0_address: tx_addr,
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

    println!(
        "started listening on rx{} and IP 172.0.0.{}",
        rx_addr.last().unwrap(),
        input_addr
    );

    tun(dev, config_rx, config_tx, delay);
}

fn tun(dev: tun::platform::Device, config_rx: RXConfig, config_tx: TXConfig, delay: u64) {
    let (reader, writer) = dev.split();
    let mut nrf_rx = NRF24L01::new(17, 0, 0).unwrap();
    nrf_rx.configure(&OperatingMode::RX(config_rx)).unwrap();
    nrf_rx.flush_input().unwrap();
    nrf_rx.listen().unwrap();

    let mut nrf_tx = NRF24L01::new(27, 1, 0).unwrap();
    nrf_tx
        .configure(&OperatingMode::TX(TXConfig { ..config_tx }))
        .unwrap();
    nrf_tx.flush_output().unwrap();

    // RX loop
    let rx = thread::spawn(move || rx_thread(writer, nrf_rx, delay));

    // TX loop
    let tx = thread::spawn(move || tx_thread(reader, config_tx, nrf_tx, delay));

    rx.join().unwrap();
    tx.join().unwrap();
}

fn rx_thread(mut writer: Writer, mut nrf_rx: NRF24L01, delay: u64) {
    println!("rx thread started!");
    let mut buf = [0u8; 4096];
    let mut end;
    let mut n_tries;
    const INITIAL_N_TRIES: i32 = 2048;

    loop {
        // Outer idle loop
        n_tries = INITIAL_N_TRIES;
        end = 0;
        sleep(Duration::from_millis(1));
        if nrf_rx.data_available().unwrap() {
            // Data should be coming in now, loop when there is data or for 64 iters
            while n_tries > 0 {
                if nrf_rx.data_available().unwrap() {
                    if end + 96 >= BUF_SIZE {
                        end = 0;
                    }

                    let n = nrf_rx
                        // read all cannot have range checks since it blocks the
                        // radio hardware hurting performance
                        .read_all(|packet| {
                            let start = end;
                            end += packet.len();
                            buf[start..end].copy_from_slice(packet);
                        })
                        .unwrap() as u32;

                    debug_println!("{} packets received", n);
                    if end > 10 {
                        let pkt = &buf[0..end];
                        // Make sure the packet is valid
                        if packet::ip::Packet::new(pkt).is_ok() {
                            let result = writer.write(pkt);
                            match result {
                                Ok(n) => debug_println!("{} bytes written to interface", n),
                                Err(err) => {
                                    debug_println!("{} error when writing to interface", err)
                                }
                            }
                            end = 0;
                            n_tries = INITIAL_N_TRIES;
                        }
                    }
                } else {
                    n_tries -= 1;
                }
                sleep(Duration::from_micros(delay));
            }
        }
    }
}

fn tx_thread(mut reader: Reader, mut config_tx: TXConfig, mut nrf_tx: NRF24L01, delay: u64) {
    println!("tx thread started!");
    let mut buf = [0u8; BUF_SIZE];

    loop {
        let read_result = reader.read(&mut buf);
        if let Ok(n) = read_result {
            if n == 0 {
                debug_println!("nothing on interface");
                continue;
            }

            let pkt = &buf[0..n];
            match packet::ip::v4::Packet::new(pkt) {
                Ok(packet) => {
                    let dst_addr = packet.destination().octets();
                    let dst = dst_addr.last().unwrap();

                    // Don't reconfigure to already set address
                    if dst != config_tx.pipe0_address.last().unwrap() {
                        let config_dst = config_tx.pipe0_address.last_mut().unwrap();
                        *config_dst = *dst;
                        debug_println!("new dst: {}", config_dst);
                        nrf_tx
                            .configure(&OperatingMode::TX(TXConfig { ..config_tx }))
                            .unwrap();
                    }
                }
                Err(_) => {
                    debug_println!("read an invalid packet from tun, dropping");
                    continue;
                }
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
                        break;
                    }
                };
                sleep(Duration::from_micros(delay / 2));
            }
        }
    }
}
