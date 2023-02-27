use std::{
    io::{Read, Write},
    thread::{self, sleep},
    time::Duration,
};

use clap::Parser;

use nrf24l01::{DataRate, OperatingMode, PALevel, RXConfig, TXConfig, NRF24L01};
use tun::platform::posix::{Reader, Writer};

extern crate tun;

const BUF_SIZE: usize = 4096;

macro_rules! debug_println {
    ($($rest:tt)*) => {
        if std::env::var("DEBUG").is_ok() {
            std::println!($($rest)*);
        }
    }
}

/// A program for sending IP traffic over NRF24L01. This program needs to be run
/// as root or have the cap_net_admin capability.
#[derive(Parser, Debug, Clone, Copy)]
#[command(author, about)]
struct Args {
    /// The address this device should listen on. Should be in range 1-254.
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..125))]
    address: u8,

    /// How long (in micros) every loop sleeps. Higher value means higher ping but less usage.
    #[arg(long, default_value_t = 20)]
    delay: u64,

    /// The MTU that should be used for the TUN interface.
    #[arg(long, default_value_t = 900, value_parser = clap::value_parser!(i32).range(500..65535))]
    mtu: i32,

    /// Makes this device tunnel all traffic through the given address.
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..125))]
    tunnel_address: Option<u8>,

    /// Max retries for the NRF24l01. Any value above 15 is capped to 15.
    #[arg(short, default_value_t = 15, value_parser = clap::value_parser!(u8).range(0..=15))]
    retries: u8,

    /// Retry delay for the NRF24l01. Any value above 15 is capped to 15.
    #[arg(short, default_value_t = 10, value_parser = clap::value_parser!(u8).range(0..=15))]
    nrf_delay: u8,
}

fn main() {
    let args = Args::parse();
    let mut rx_addr = *b"rx0";

    *rx_addr.last_mut().unwrap() = args.address;

    let mut config = tun::Configuration::default();
    config
        .name("longge")
        .address((172, 0, 0, args.address))
        .netmask((255, 255, 255, 0))
        .mtu(args.mtu)
        .up();

    let dev = tun::create(&config).unwrap();

    if args.tunnel_address.is_some() {
        std::fs::write("/proc/sys/net/ipv4/ip_forward", "1").expect("could not enable ipv4 forwarding");

        std::process::Command::new("iptables")
            .args([
                "-A", "FORWARD", "-i", "longge", "-o", "eth0", "-j", "ACCEPT",
            ])
            .output()
            .unwrap();
        std::process::Command::new("iptables")
            .args([
                "-A", "FORWARD", "-i", "longge", "-o", "wlan0", "-j", "ACCEPT",
            ])
            .output()
            .unwrap();
        std::process::Command::new("iptables")
            .args([
                "-A",
                "FORWARD",
                "-i",
                "eth0",
                "-o",
                "longge",
                "-m",
                "state",
                "--state",
                "RELATED,ESTABLISHED",
                "-j",
                "ACCEPT",
            ])
            .output()
            .unwrap();
        std::process::Command::new("iptables")
            .args([
                "-A",
                "FORWARD",
                "-i",
                "wlan0",
                "-o",
                "longge",
                "-m",
                "state",
                "--state",
                "RELATED,ESTABLISHED",
                "-j",
                "ACCEPT",
            ])
            .output()
            .unwrap();
        std::process::Command::new("iptables")
            .args([
                "-t",
                "nat",
                "-A",
                "POSTROUTING",
                "-o",
                "eth0",
                "-j",
                "MASQUERADE",
            ])
            .output()
            .unwrap();
        std::process::Command::new("iptables")
                .args([
                    "-t",
                    "-nat",
                    "-A",
                    "POSTROUTING",
                    "-o",
                    "wlan0",
                    "-j",
                    "MASQUERADE"
                ]).output()
                .unwrap();

        // Teardown all rules on program exit
        ctrlc::set_handler(|| {
            std::process::Command::new("iptables")
                .args([
                    "-D", "FORWARD", "-i", "longge", "-o", "eth0", "-j", "ACCEPT",
                ])
                .output()
                .unwrap();
            std::process::Command::new("iptables")
                .args([
                    "-D", "FORWARD", "-i", "longge", "-o", "wlan0", "-j", "ACCEPT",
                ])
                .output()
                .unwrap();
            std::process::Command::new("iptables")
                .args([
                    "-D",
                    "FORWARD",
                    "-i",
                    "eth0",
                    "-o",
                    "longge",
                    "-m",
                    "state",
                    "--state",
                    "RELATED,ESTABLISHED",
                    "-j",
                    "ACCEPT",
                ])
                .output()
                .unwrap();
            std::process::Command::new("iptables")
                .args([
                    "-D",
                    "FORWARD",
                    "-i",
                    "wlan0",
                    "-o",
                    "longge",
                    "-m",
                    "state",
                    "--state",
                    "RELATED,ESTABLISHED",
                    "-j",
                    "ACCEPT",
                ])
                .output()
                .unwrap();
            std::process::Command::new("iptables")
                .args([
                    "-t",
                    "nat",
                    "-D",
                    "POSTROUTING",
                    "-o",
                    "eth0",
                    "-j",
                    "MASQUERADE",
                ])
                .output()
                .unwrap();
            std::process::Command::new("iptables")
                .args([
                    "-t",
                    "nat",
                    "-D",
                    "POSTROUTING",
                    "-o",
                    "wlan0",
                    "-j",
                    "MASQUERADE",
                ])
                .output()
                .unwrap();

            std::fs::write("/proc/sys/net/ipv4/ip_forward", "0").expect("could not disable ipv4 forwarding");
        }).unwrap();
    }

    let config_rx = RXConfig {
        channel: args.address,
        pa_level: PALevel::Low,
        pipe0_address: rx_addr,
        data_rate: DataRate::R2Mbps,
        ..Default::default()
    };
    let mut tx_addr = rx_addr;
    *tx_addr.last_mut().unwrap() += 1;
    let config_tx = TXConfig {
        channel: args.address + 1,
        pa_level: PALevel::Low,
        pipe0_address: tx_addr,
        max_retries: args.retries,
        retry_delay: args.nrf_delay,
        data_rate: DataRate::R2Mbps,
    };

    assert!(
        config_rx.data_rate == DataRate::R2Mbps
            && config_rx.data_rate == config_tx.data_rate
            && config_rx.channel + 1 == config_tx.channel
            && config_rx.pipe0_address != config_tx.pipe0_address
    );

    println!(
        "started listening on rx{} and IP 172.0.0.{}",
        rx_addr.last().unwrap(),
        args.address
    );

    tun(dev, config_rx, config_tx, args);
}

fn tun(dev: tun::platform::Device, config_rx: RXConfig, config_tx: TXConfig, args: Args) {
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
    let rx = thread::spawn(move || rx_thread(writer, nrf_rx, args));

    // TX loop
    let tx = thread::spawn(move || tx_thread(reader, config_tx, nrf_tx, args));

    rx.join().unwrap();
    tx.join().unwrap();
}

fn rx_thread(mut writer: Writer, mut nrf_rx: NRF24L01, args: Args) {
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
                sleep(Duration::from_micros(args.delay / 2));
            }
        }
    }
}

fn tx_thread(mut reader: Reader, mut config_tx: TXConfig, mut nrf_tx: NRF24L01, args: Args) {
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
                    // Only set addresses when not using tunnel_address
                    if args.tunnel_address.is_none() {
                        let dst_addr = packet.destination().octets();
                        let dst = dst_addr.last().unwrap();

                        // Don't reconfigure to already set address
                        if dst != config_tx.pipe0_address.last().unwrap() {
                            let config_dst = config_tx.pipe0_address.last_mut().unwrap();
                            *config_dst = *dst;
                            config_tx.channel = *dst;
                            debug_println!("new dst: {}", config_dst);
                            nrf_tx
                                .configure(&OperatingMode::TX(TXConfig { ..config_tx }))
                                .unwrap();
                        }
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
                // Try two times in software
                match nrf_tx.send() {
                    Ok(retries) => debug_println!("message sent, {} retries needed", retries),
                    Err(err) => {
                        debug_println!("destination unreachable: {:?}", err);
                        match nrf_tx.send() {
                            Ok(retries) => {
                                debug_println!("message sent, {} retries needed", retries)
                            }
                            Err(err) => {
                                debug_println!("destination unreachable: {:?}", err);
                                nrf_tx.flush_output().unwrap();
                                break;
                            }
                        };
                    }
                };
                sleep(Duration::from_micros(args.delay));
            }
        }
    }
}
