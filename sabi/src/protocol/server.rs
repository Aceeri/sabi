use bevy::prelude::*;
use bevy_renet::renet::{RenetServer, ServerConfig};

use std::{
    net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use std::time::SystemTime;

use crate::protocol::*;

pub fn new_renet_server() -> RenetServer {
    let local_ip =
        crate::protocol::public_ip().unwrap_or(crate::protocol::localhost_ip().to_owned());
    let port = crate::protocol::PORT;

    let mut public_ip = None;
    // Set up ports using UPnP so people don't have to port forward.
    match igd::search_gateway(igd::SearchOptions {
        timeout: Some(Duration::from_secs(1)),
        ..Default::default()
    }) {
        Err(ref err) => println!("Error: {}", err),
        Ok(gateway) => {
            let local_addr = local_ip.parse::<Ipv4Addr>().unwrap();
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

            match gateway.get_external_ip() {
                Ok(external_ip) => {
                    public_ip = Some(external_ip);
                }
                Err(ref err) => {
                    error!("get_external_ip: {}", err);
                }
            }
        }
    };

    let external_ip = match public_ip {
        Some(ip) => ip.to_string(),
        None => my_internet_ip::get()
            .expect("failed to get external ip, cannot start server")
            .to_string(),
    };

    let server_addr = format!("{}:{}", external_ip, crate::protocol::PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    let local_addr = format!("{}:{}", local_ip, crate::protocol::PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    println!("binding to {:?}", server_addr);
    let protocol_id = crate::protocol::protocol_id();
    println!("protocol id: {:?}", protocol_id,);

    let socket = UdpSocket::bind(local_addr).unwrap();
    socket
        .set_nonblocking(true)
        .expect("Can't set non-blocking mode");

    let connection_config = crate::protocol::renet_connection_config();
    let server_config = ServerConfig {
        max_clients: 10,
        protocol_id: protocol_id,
        private_key: *PRIVATE_KEY,
    };
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    RenetServer::new(current_time, server_config, connection_config, socket).unwrap()
}

/*
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct BandwidthTimer(Timer);

impl BandwidthTimer {
    pub fn new() -> Self {
        Self(Timer::new(Duration::from_secs(1), true))
    }
}

pub fn display_server_bandwidth(
    time: Res<Time>,
    lobby: Res<Lobby>,
    mut timer: ResMut<BandwidthTimer>,
    server: ResMut<RenetServer>,
) {
    timer.tick(time.delta());

    if timer.just_finished() {
        for client_id in lobby.players.keys() {
            if let Some(network_info) = server.network_info(*client_id) {
                info!(
                    "client: {}, rtt: {:.0}, loss: {:.2}, skbps: {:.1}, rkbps: {:.1}",
                    client_id,
                    network_info.rtt,
                    network_info.packet_loss,
                    network_info.sent_bandwidth_kbps,
                    network_info.received_bandwidth_kbps,
                );
            }
        }
    }
}
 */
