use std::net::UdpSocket;

pub fn main() {
    let socket = UdpSocket::bind("192.168.0.110:0").unwrap();
    dbg!(socket.local_addr());
    dbg!(socket.peer_addr());
    let local_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    dbg!(local_socket.local_addr());
}
