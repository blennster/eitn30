use packet::{icmp, ip, Builder, Packet};

pub trait VirtHost {
    fn get_addr(&self) -> u8;
    fn set_addr(&mut self, addr: u8);
    fn send(&self, data: &[u8], dst: u8);
    fn rec(&mut self) -> &[u8];
}

pub fn icmp_reply(pkt: &[u8]) -> Result<Vec<u8>, String> {
    match ip::Packet::new(pkt) {
        Ok(ip::Packet::V4(pkt)) => {
            if let Ok(icmp) = icmp::Packet::new(pkt.payload()) {
                match icmp.echo() {
                    Ok(icmp) => {
                        let reply = ip::v4::Builder::default()
                            .id(0x42)
                            .unwrap()
                            .ttl(64)
                            .unwrap()
                            .source(pkt.destination())
                            .unwrap()
                            .destination(pkt.source())
                            .unwrap()
                            .icmp()
                            .unwrap()
                            .echo()
                            .unwrap()
                            .reply()
                            .unwrap()
                            .identifier(icmp.identifier())
                            .unwrap()
                            .sequence(icmp.sequence())
                            .unwrap()
                            .payload(icmp.payload())
                            .unwrap()
                            .build()
                            .unwrap();
                        return Ok(reply);
                    }
                    Err(_) => return Err("general error".to_string()),
                }
            }
        }
        Err(err) => return Err(err.to_string()),
        _ => return Err("general error".to_string()),
    };

    Err("general".to_string())
}
