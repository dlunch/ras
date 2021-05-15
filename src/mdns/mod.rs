mod packet;

use async_std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use async_std::io;

pub fn cast<T>(data: &[u8]) -> &T {
    unsafe { &*(data.as_ptr() as *const T) }
}

pub async fn serve(service_name: &str) -> io::Result<()> {
    let any = Ipv4Addr::new(0, 0, 0, 0);
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);

    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(any), 5353)).await?;
    socket.join_multicast_v4(mdns_addr, any)?;

    loop {
        let mut header_buf = [0; 12];
        socket.recv_from(&mut header_buf).await?;

        let header = cast::<packet::Header>(&header_buf);

        println!("{:?}", header);
    }

    Ok(())
}
