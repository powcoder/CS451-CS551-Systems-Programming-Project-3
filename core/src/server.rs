https://powcoder.com
代写代考加微信 powcoder
Assignment Project Exam Help
Add WeChat powcoder
use crate::cmd::Command;
use crate::cmd::Ping;
use crate::cmd::Pong;
use crate::cmd::Register;
use crate::cmd::Whisper;
use crate::connection::ConnectionData;
use crate::Bing2BingError;
use crate::{
    cmd::{Announce, Say},
    peer::PeerData,
    util::{ConnectionCounter, SequenceNumberGenerator},
    ClientServerMessage, ClientTxChannel, Peer, ServerRxChannel,
};
use crate::{
    parser::{ParseError, Parser},
    peer_map::PeerMap,
    Bing2BingFrame, Connection,
};
use crate::{util::TtlMap, Bing2BingCommand};
use std::convert::TryInto;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, instrument, trace};

/// The "server" side of the P2P chat application.
/// A server is primarily focused around network related activity and manages most everything related to the protocol itself.
/// This includes receiving commands over the network, processing them, and sending commands out to the network.
/// The server also receives messages from its corresponding [Client](crate::Client) which is what the end user will be interacting with.
#[derive(Debug)]
pub struct Server {
    /// What to listen on
    listener: TcpListener,
    /// Sequence number generator
    sequence_numbers: SequenceNumberGenerator,
    name: String,
    ip_address: String,
    port: u64,
    num_incoming_conns: ConnectionCounter,
    client_tx: ClientTxChannel,
    rx: ServerRxChannel,
    my_peer_data: PeerData,
}

impl Server {
    pub async fn new(
        name: &str,
        bind_address: &str,
        port: &str,
        client_tx: ClientTxChannel,
        rx: ServerRxChannel,
    ) -> Result<Self, Bing2BingError> {
        let my_peer_data = PeerData::default();
        Ok(Server {
            listener: TcpListener::bind(format!("{}:{}", bind_address, port)).await?,
            sequence_numbers: SequenceNumberGenerator::new(0),
            name: name.to_string(),
            ip_address: bind_address.to_string(),
            port: port.to_string().parse().unwrap(),
            num_incoming_conns: ConnectionCounter::new(0),
            client_tx,
            rx,
            my_peer_data,
        })
    }

    /// Begin listening for inbound connections.
    #[instrument(level = "trace")]
    pub async fn listen(
        &self,
        peer_map: &PeerMap,
        client_tx: ClientTxChannel,
    ) -> Result<(), Bing2BingError> {
        let peers = peer_map;
        let adjacency_list: TtlMap<PeerData> = TtlMap::new();
        let processed_commands: TtlMap<bool> = TtlMap::new();

        loop {
            let (stream, addr) = self.listener.accept().await?;

            let peers = peers.clone(); //Arc::clone(&peers);
            let adjacency_list = adjacency_list.clone();

            let processed_commands = processed_commands.clone();
            let connection_counter = self.num_incoming_conns.clone();

            let client_tx = client_tx.clone();

            let my_name = self.name.clone();
            let sequence_numbers = self.sequence_numbers.clone();

            tokio::spawn(async move {
                trace!("Accepted connection from {:?}", addr);

                connection_counter.inc();

                let peering_info = PeeringInfo {
                    peers: &peers,
                    adjacency_list,
                    processed_commands,
                };

                let connection_handler = Server::handle_connection(
                    peering_info,
                    stream,
                    addr,
                    client_tx,
                    &my_name,
                    sequence_numbers,
                );

                connection_handler.await.unwrap_or_else(|err| {
                    trace!(
                        "An error occurred: {:?}; incoming connection disconnected?",
                        err
                    );
                });

                connection_counter.dec();
            });
        }
    }

    /// Handles an incomming connection. I.e., another peer that has initiated a connection with us.
    /// In particular, this method reads command frames from a [Connection], checks to make sure that the
    /// command hasn't already been processed, and if it hasn't, processes the command.
    /// This method will also pass relevant [ClientServerMessage]s up to a
    /// [Client](crate::Client) for further use.
    #[instrument(level = "trace")]
    pub(crate) async fn handle_connection(
        // peers: &PeerMap,
        // adjacency_list: TtlMap<PeerData>,
        // processed_commands: TtlMap<bool>,
        peering_info: PeeringInfo<'_>,
        stream: TcpStream,
        addr: SocketAddr,
        client_tx: ClientTxChannel,
        my_name: &str,
        sequence_numbers: SequenceNumberGenerator,
    ) -> Result<(), Bing2BingError> {
        let mut connection_data = ConnectionData {
            peers: peering_info.peers,
            adjacency_list: peering_info.adjacency_list,
            connection: Connection::new(stream).await,
        };

        loop {
            let frame = connection_data.connection.read_frame().await?;
            trace!("Received {:?} from {}", frame, addr);

            let frame = match frame {
                Some(frame) => frame,
                None => {
                    trace!("Connection ended?");
                    break;
                }
            };

            // we expect to only see Command frames at this point.
            // let command = Bing2BingCommand::from_frame(frame)?;
            let command: Bing2BingCommand = frame.try_into()?;

            trace!(?command);

            // let's see if we've already processed this commmand.
            if command.check_duplicate(&peering_info.processed_commands) {
                continue;
            }

            command.set_processed(&peering_info.processed_commands);

            // now see which command it was and apply it.
            // this could be refactored to another function to make life easier
            // perhaps?
            match command {
                Bing2BingCommand::Pong(cmd) => {
                    debug!(
                        "Received a Pong command on an incoming connection: {:?}",
                        cmd
                    );
                    if cmd.destination == my_name {
                        // we were the destination, so just fire off an event to the client
                        debug!("We were destination for the pong commnd: {:?}", cmd);
                        client_tx
                            .send(ClientServerMessage::Pong((cmd.source, cmd.sent_at)))
                            .await?;
                    } else {
                        cmd.apply(&mut connection_data).await?
                    }
                }
                Bing2BingCommand::Ping(cmd) => {
                    debug!(
                        "Received a Ping command on an incoming connection: {:?}",
                        cmd
                    );
                    if cmd.destination == my_name {
                        // we were the destination, so just fire off an event to the client
                        debug!("We were destination for the Ping commnd: {:?}", cmd);
                        // now we need to send a pong response
                        let sequence_number = sequence_numbers.next();
                        let pong = Pong::new(my_name, &cmd.source, sequence_number, cmd.sent_at);
                        debug!("applying pong command: {:?}", pong);
                        pong.apply(&mut connection_data).await?
                    } else {
                        // we weren't destination, just forward the ping along.
                        cmd.apply(&mut connection_data).await?
                    }
                    // cmd.apply(&mut connection_data).await?
                }
                Bing2BingCommand::Say(cmd) => {
                    trace!("Received a Say command on an incoming connection");
                    trace!("Sending to client");
                    client_tx
                        .send(ClientServerMessage::Say((
                            cmd.source.clone(),
                            cmd.message.clone(),
                        )))
                        .await?;
                    cmd.apply(&mut connection_data).await?;
                }
                Bing2BingCommand::Announce(cmd) => cmd.apply(&mut connection_data).await?,
                Bing2BingCommand::Broadcast(cmd) => cmd.apply(&mut connection_data).await?,
                Bing2BingCommand::Deliver(cmd) => cmd.apply(&mut connection_data).await?,
                Bing2BingCommand::Whisper(cmd) => {
                    trace!(
                        "Received a Whisper command on an incoming connection: {:?}",
                        cmd
                    );
                    if cmd.destination == my_name {
                        // we were the destination, so just fire off an event to the client
                        trace!("We were destination for whisper commnd: {:?}", cmd);
                        client_tx
                            .send(ClientServerMessage::Whisper((
                                cmd.source,
                                cmd.destination,
                                cmd.message,
                            )))
                            .await?;
                    } else {
                        cmd.apply(&mut connection_data).await?
                    }
                }
                Bing2BingCommand::Extension(cmd) => cmd.apply(&mut connection_data).await?,
                Bing2BingCommand::Register(cmd) => {
                    tracing::error!(
                        "REGISTER COMMAND NOT IMPLEMENTED BY DEFAULT ON SERVERS (peers) {:?}",
                        cmd
                    )
                }
                Bing2BingCommand::Unknown => {
                    tracing::trace!("Received unimplemented command! {:?}", command)
                }
            }
        }

        Ok(())
    }

    /// Convienence function that broadcasts a say message.
    /// This is useful for handling messages that are coming in from the associated [Client](crate::Client).
    /// I.e., our user wants to say something.
    pub async fn say(peer_map: &PeerMap, from: String, message: String, sequence_number: u64) {
        let frame = Say::new(from.to_string(), sequence_number, &message).into_frame();

        peer_map.broadcast(&from, frame);
    }

    /// Convienence function that sends a ping message
    pub async fn ping(
        peer_map: &PeerMap,
        from: &str,
        to: &str,
        sequence_number: u64,
        sent_at: u64,
    ) {
        let frame = Ping::new(from, to, sequence_number, sent_at).into();

        peer_map.broadcast(from, frame);
    }

    /// Convienence function that sends a ping message
    pub async fn pong(
        peer_map: &PeerMap,
        from: &str,
        to: &str,
        sequence_number: u64,
        sent_at: u64,
    ) {
        let frame = Pong::new(from, to, sequence_number, sent_at).into();

        peer_map.broadcast(from, frame);
    }

    /// Convienence function that sends a whisper message.
    /// This is useful for handling messages that are coming in from the associated [Client](crate::Client).
    /// I.e., our user wants to whisper something.
    pub async fn whisper(
        peer_map: &PeerMap,
        from: &str,
        to: &str,
        message: &str,
        sequence_number: u64,
    ) {
        let frame = Whisper::new(from, sequence_number, to, message).into();

        peer_map.broadcast(from, frame);
    }

    /// Convienence function that gets the next sequence number for a message originating from this peer.
    fn next_sequence_number(&self) -> u64 {
        self.sequence_numbers.next()
    }

    /// Starts the server.
    /// This is primarily three steps:
    ///
    /// 1. We want to register with the tracker.
    /// 2. We want to connect to peers that we get back from the tracker.
    /// 3. We want to start listening for incoming connections from other peers.
    /// 4. We want to start announcing our neighborhood to others.
    #[instrument(level = "trace")]
    pub async fn start(
        &self,
        tracker_ip: &str,
        tracker_port: &str,
        max_incoming_connections: u64,
        // next_sequence_number: Arc<Mutex<u64>>,
    ) -> Result<(), Bing2BingError> {
        // 1) we want to connect to tracker.
        // 2) we want to connect to peers
        // 3) we want to start listening for incoming connections.

        // Connect to tracker
        let tracker_addr = format!("{}:{}", tracker_ip, tracker_port);
        let tcp_stream = TcpStream::connect(tracker_addr).await?;
        let mut connection = Connection::new(tcp_stream).await;

        let sequence_number = self.next_sequence_number();

        let frame = Register::new(
            &self.name,
            sequence_number,
            &self.ip_address,
            &self.port.to_string(),
        )
        .into_frame();

        // lock is released; we have a "guaranteed unique" sequence number
        connection.write_frame(frame).await.unwrap();

        let response_frame = connection.read_frame().await.unwrap().unwrap();
        let received_peers = self.parse_register_response(response_frame)?;
        trace!("received peers from announce: {:?}", received_peers);
        let peer_map = PeerMap::default();

        // we need to add each of these to the peer map.
        for (peer_name, ip_address, port) in received_peers {
            trace!("Adding peer {} from Register list", peer_name);
            if peer_name != self.name {
                Server::connect_to_peer(&peer_map, peer_name, ip_address, port);
            }
        }

        // let next_sequence_number = self.sequence_numbers.clone();
        let peer_map_move = peer_map.clone();

        self.client_message_handler(&peer_map_move, self.rx.clone());

        // start up an announce task
        let next_sequence_number = self.sequence_numbers.clone();
        let peer_map_move = peer_map.clone();

        let name = self.name.clone();
        // let port = self.port;
        let ip_address = self.ip_address.clone();

        let port = self.port;

        let num_incoming_conns = self.num_incoming_conns.clone();

        // POINTS AVAILABLE
        // this might be fine just doing a tokio spawn instead of a thread.
        let my_peer_data = self.my_peer_data.clone();
        std::thread::spawn(move || {
            start_announce(
                name,
                ip_address,
                port,
                &peer_map_move,
                next_sequence_number,
                num_incoming_conns,
                max_incoming_connections,
                my_peer_data,
            )
        });

        self.listen(&peer_map, self.client_tx.clone()).await
    }

    /// This method handles messages that come in from the associated [`Client`](crate::Client)
    #[instrument(level = "trace")]
    fn client_message_handler(&self, peer_map: &PeerMap, rx: ServerRxChannel) {
        let peer_map = peer_map.clone();
        let next_sequence_number = self.sequence_numbers.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(msg) = rx.recv().await {
                    trace!("Server received {:?} from client", msg);
                    match msg {
                        ClientServerMessage::Ping((from, to, sent_at)) => {
                            debug!("matched a ClientServerMessage::Ping message");
                            let sequence_number = next_sequence_number.next();
                            Server::ping(&peer_map, &from, &to, sequence_number, sent_at).await;
                        }
                        ClientServerMessage::Pong(_) => {
                            panic!(
                                "We received a Pong message from client, but that makes no sense!"
                            );
                        }
                        ClientServerMessage::Whisper((from, to, message)) => {
                            trace!("matched a ClientServerMessage::Whisper message");
                            let sequence_number = next_sequence_number.next();
                            trace!("Executing Server::whisper");
                            Server::whisper(&peer_map, &from, &to, &message, sequence_number).await;
                        }
                        ClientServerMessage::Say((from, message)) => {
                            trace!("matched a  ClientServerMessage::Say message");
                            // we should do a say.
                            let sequence_number = next_sequence_number.next();

                            trace!("exceutiong Server::say");

                            Server::say(&peer_map, from, message, sequence_number).await;
                        }
                    }
                }
            }
        });
    }

    fn parse_register_response(
        &self,
        response: Bing2BingFrame,
    ) -> Result<Vec<(String, String, String)>, Bing2BingError> {
        let mut parse = Parser::new(response)?;

        let mut ret: Vec<(String, String, String)> = Vec::new();

        loop {
            match parse.next() {
                Ok(Bing2BingFrame::Array(array)) => {
                    // POINTS AVAILABLE
                    // i don't think i should have to deconstruct and then reconstruct
                    // this, although i'm not sure how to deal with it better.
                    let array = Bing2BingFrame::Array(array);

                    let mut peer_info_parse = Parser::new(array)?;

                    let peer_name = peer_info_parse.next_text()?;
                    let ip_address = peer_info_parse.next_text()?;
                    let port = peer_info_parse.next_text()?;

                    peer_info_parse.finish()?;
                    ret.push((peer_name, ip_address, port));
                }
                Err(ParseError::EndOfStream) => break,
                Err(err) => return Err(Box::new(err)),
                _ => {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Found a tracker register response that was not an array!",
                    )))
                }
            }
        }

        Ok(ret)
    }

    #[instrument(level = "trace")]
    pub(crate) fn connect_to_peer(
        peer_map: &PeerMap,
        peer_name: String,
        ip_address: String,
        port: String,
    ) {
        let mut peer_map = peer_map.clone();

        tokio::spawn(async move {
            let (peer_tx, peer_rx) = mpsc::unbounded_channel();

            // POINTS AVAILABLE
            // It is likely possible to remove all these clones with some refactoring, but I got lazy
            let mut peer = Peer::new(peer_name.clone(), ip_address.clone(), port.clone(), peer_rx);

            peer_map.insert(peer_name.clone(), peer_tx);

            peer.start()
                .await
                .unwrap_or_else(|x| debug!("peer.start() errored out {}", x));

            // this is not the greatest way to handle a disconnect coming from a peer
            // but, we could send a [Peer] a [PeerControlMessage::ShutDown] and then it should break from the loop
            peer_map.remove(peer_name.clone());
        });
    }
}

/// *POINTS AVAILABLE*
/// Right now, this method will announce the peer to the rest
/// of the network every 5 seconds.
/// As part of this announcement, the peer will transmit the name of the city
/// it's in, as well as lat and longitude.
/// It would be nice to have this be configurable instead of hard coded
/// as it currently is.
#[tokio::main]
#[instrument(level = "trace")]
async fn start_announce(
    name: String,
    ip_address: String,
    port: u64,
    peer_map: &PeerMap,
    next_sequence_number: SequenceNumberGenerator,
    num_incoming_conns: ConnectionCounter,
    max_incoming_conns: u64,
    my_peer_data: PeerData,
) {
    let city = my_peer_data.city;
    let lat = my_peer_data.lat;
    let lng = my_peer_data.lng;
    loop {
        let sequence_number = next_sequence_number.next();

        let peers = peer_map.peer_names();

        let num_incoming_conns = num_incoming_conns.get();

        let available_incoming = match num_incoming_conns < max_incoming_conns {
            true => max_incoming_conns - num_incoming_conns,
            false => 0,
        };

        // POINTS AVAILABLE
        // Try making a configuration setting that does this with some more
        // configurability
        let announce = Announce::new(
            &name,
            sequence_number,
            &ip_address,
            port,
            available_incoming,
            &city,
            lat,
            lng,
            peers,
        );

        let announce_frame = announce.into_frame();
        trace!("Broadcasting announce frame: {:?}", announce_frame);

        peer_map.broadcast(&name, announce_frame);

        trace!("announce sleeping");
        tokio::time::sleep(Duration::from_secs(5)).await;
        trace!("announce woke up!");
    }
}

fn peering_info_test<'a, 'b: 'a>(peers: &'b PeerMap) -> PeeringInfo<'a> {
    // let peers = PeerMap::new();

    let adjacency_list: TtlMap<PeerData> = TtlMap::new();

    let processed_commands: TtlMap<bool> = TtlMap::new();

    let peering_info = PeeringInfo {
        peers: peers,
        adjacency_list,
        processed_commands,
    };

    // drop(peers);

    println!("{peering_info:?}");

    peering_info
}

#[derive(Debug)]
struct PeeringInfo<'peer_map> {
    peers: &'peer_map PeerMap,
    adjacency_list: TtlMap<PeerData>,
    processed_commands: TtlMap<bool>,
}

// peers: &PeerMap,
// adjacency_list: TtlMap<PeerData>,
// processed_commands: TtlMap<bool>,
// stream: TcpStream,
// addr: SocketAddr,
// client_tx: ClientTxChannel,
// my_name: &str,
// sequence_numbers: SequenceNumberGenerator,
