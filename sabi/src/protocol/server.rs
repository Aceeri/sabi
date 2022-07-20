use bevy::prelude::*;
use bevy_renet::renet::{RenetServer, ServerAuthentication, ServerConfig};

use std::{
    error::Error,
    net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use std::time::SystemTime;

use crate::protocol::*;

pub fn new_renet_server<S: AsRef<str>>(
    local_ip: S,
    mut public_ip: Option<String>,
    port: u16,
) -> Result<RenetServer, Box<dyn Error>> {
    let local_ip = local_ip.as_ref();

    if local_ip == "127.0.0.1" || local_ip == "0.0.0.0" {
        public_ip = Some("127.0.0.1".to_owned());
    }

    // Set up ports using UPnP so people don't have to port forward.
    if let None = public_ip {
        match igd::search_gateway(igd::SearchOptions {
            timeout: Some(Duration::from_secs(5)),
            ..Default::default()
        }) {
            Err(ref err) => println!("Error: {}", err),
            Ok(gateway) => {
                let local_addr = local_ip.parse::<Ipv4Addr>()?;
                let local_addr = SocketAddrV4::new(local_addr, port);

                match gateway.add_port(
                    igd::PortMappingProtocol::UDP,
                    port,
                    local_addr,
                    0,
                    "add_port example",
                ) {
                    Ok(()) => {
                        info!("Forwarded port {} to {}", port, local_addr);
                    }
                    Err(ref err) => {
                        error!("failed to add port to gateway: {}", err);
                    }
                }

                public_ip = Some(gateway.get_external_ip()?.to_string());
            }
        };
    }

    if let None = public_ip {
        if let Ok(ip) = my_internet_ip::get() {
            public_ip = Some(ip.to_string());
        }
    }

    let server_addr = format!("{}:{}", public_ip.ok_or("expected a public ip")?, port)
        .to_socket_addrs()?
        .next()
        .ok_or(SabiError::NoSocketAddr)?;

    let local_addr = format!("{}:{}", local_ip, port)
        .to_socket_addrs()?
        .next()
        .ok_or(SabiError::NoSocketAddr)?;

    println!("binding to {:?}", server_addr);
    let protocol_id = crate::protocol::protocol_id();
    println!("protocol id: {:?}", protocol_id,);

    let socket = UdpSocket::bind(local_addr)?;
    socket.set_nonblocking(true)?;

    let connection_config = crate::protocol::server_renet_config();
    let server_config = ServerConfig {
        max_clients: 10,
        protocol_id: protocol_id,
        public_addr: server_addr,
        authentication: ServerAuthentication::Secure {
            private_key: *PRIVATE_KEY,
        },
    };
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    Ok(RenetServer::new(
        current_time,
        server_config,
        connection_config,
        socket,
    )?)
}
