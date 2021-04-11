/*!
On Android, we can only share maps between processes by serializing fds over sockets.
Hence, the `ashmem_server` keeps track of existing maps, creates new maps for clients,
and forwards them over unix domain sockets.
*/

use crate::{
    bolts::shmem::{ShMem, ShMemDescription, UnixShMem},
    Error,
};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    io::{ErrorKind, Read, Write},
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Arc, Condvar, Mutex,
    },
};

#[cfg(all(feature = "std", unix))]
use nix::poll::{poll, PollFd, PollFlags};

#[cfg(all(feature = "std", unix))]
use std::{
    os::unix::{
        net::{UnixListener, UnixStream},
        {io::AsRawFd, prelude::RawFd},
    },
    thread,
};

#[cfg(all(unix, feature = "std"))]
use uds::{UnixListenerExt, UnixSocketAddr, UnixStreamExt};

#[derive(Debug)]
/// The Sharedmem backed by a `ShmemService`
pub enum ServedShMem {
    /// This is the server
    Server(ServedShMemServer),
    /// We're a client
    Client(ServedShMemClient),
}

#[derive(Debug)]
pub struct ServedShMemClient {
    stream: UnixStream,
    shmem: Option<UnixShMem>,
    slice: Option<[u8; 20]>,
    fd: Option<RawFd>,
}

impl ServedShMemClient {
    /// Send a request to the server, and wait for a response
    fn send_receive(&mut self, request: AshmemRequest) -> Result<([u8; 20], RawFd), crate::Error> {
        let body = postcard::to_allocvec(&request).unwrap();

        let header = (body.len() as u32).to_be_bytes();
        let mut message = header.to_vec();
        message.extend(body);

        self.stream
            .write_all(&message)
            .expect("Failed to send message");

        let mut shm_slice = [0u8; 20];
        let mut fd_buf = [-1; 1];
        self.stream
            .recv_fds(&mut shm_slice, &mut fd_buf)
            .expect("Did not receive a response");
        Ok((shm_slice, fd_buf[0]))
    }
}

#[cfg(unix)]
extern "C" {
    #[cfg(feature = "std")]
    fn snprintf(_: *mut u8, _: usize, _: *const u8, _: ...) -> u32;
}

#[derive(Debug)]
pub struct ServedShMemServer {
    shmem: Option<UnixShMem>,
}

const ASHMEM_SERVER_NAME: &str = "@ashmem_server";

impl ServedShMem {
    /// Create a new ServedShMem. If the server can be reached, a ServedShMem::Client is created
    /// and connected to the server. If not, the ServedShMemServer is spawned.
    pub fn new(name: &str) -> Result<Self, crate::Error> {
        match UnixStream::connect_to_unix_addr(&UnixSocketAddr::new(name).unwrap()) {
            Ok(stream) => Ok(Self::Client(ServedShMemClient {
                stream,
                shmem: None,
                slice: None,
                fd: None,
            })),
            Err(err) => {
                panic!("we shouldn't reaach here {:?}", backtrace::Backtrace::new());
                if err.kind() == ErrorKind::ConnectionRefused {
                    //dbg!("creating server: {:?}", backtrace::Backtrace::new());
                    Ok(Self::Server(ServedShMemServer { shmem: None }))
                } else {
                    Err(Error::Unknown("".to_string()))
                }
            }
        }
    }
}

impl ShMem for ServedShMem {
    fn new_map(map_size: usize) -> Result<Self, crate::Error> {
        match Self::new(ASHMEM_SERVER_NAME).unwrap() {
            ServedShMem::Client(mut client) => {
                match client.send_receive(AshmemRequest::NewMap(map_size)) {
                    Ok((shm_slice, fd)) => {
                        client.slice = Some(shm_slice);
                        client.fd = Some(fd);

                        let mut ourkey: [u8; 20] = [0; 20];
                        unsafe {
                            snprintf(
                                ourkey.as_mut_ptr() as *mut u8,
                                20,
                                b"%d\x00" as *const u8,
                                fd,
                            );
                        }
                        client.shmem = Some(
                            UnixShMem::existing_from_shm_slice(&ourkey, map_size)
                                .expect("Failed to create the UnixShMem"),
                        );
                        Ok(ServedShMem::Client(client))
                    }
                    Err(e) => Err(e),
                }
            }
            ServedShMem::Server(mut server) => {
                server.shmem =
                    Some(UnixShMem::new(map_size).expect("Failed to create the UnixShMem"));
                Ok(ServedShMem::Server(server))
            }
        }
    }

    fn existing_from_shm_slice(
        map_str_bytes: &[u8; 20],
        map_size: usize,
    ) -> Result<Self, crate::Error> {
        match Self::new(ASHMEM_SERVER_NAME).unwrap() {
            ServedShMem::Client(mut client) => {
                let (shm_slice, fd) = client
                    .send_receive(AshmemRequest::ExistingMap(ShMemDescription {
                        size: map_size,
                        str_bytes: *map_str_bytes,
                    }))
                    .expect("Could not allocate from the ashmem server");

                let mut ourkey: [u8; 20] = [0; 20];
                unsafe {
                    snprintf(
                        ourkey.as_mut_ptr() as *mut u8,
                        20,
                        b"%d\x00" as *const u8,
                        fd,
                    );
                }
                client.slice = Some(shm_slice);
                client.fd = Some(fd);
                client.shmem = Some(
                    UnixShMem::existing_from_shm_slice(&ourkey, map_size)
                        .expect("Failed to create the UnixShMem"),
                );
                Ok(ServedShMem::Client(client))
            }
            ServedShMem::Server(mut server) => {
                server.shmem = Some(
                    UnixShMem::existing_from_shm_slice(map_str_bytes, map_size)
                        .expect("Failed to create the UnixShMem"),
                );
                Ok(ServedShMem::Server(server))
            }
        }
    }

    fn shm_slice(&self) -> &[u8; 20] {
        match self {
            ServedShMem::Client(client) => client.slice.as_ref().unwrap(),
            ServedShMem::Server(server) => server.shmem.as_ref().unwrap().shm_slice(),
        }
    }

    fn map(&self) -> &[u8] {
        match self {
            ServedShMem::Client(client) => client.shmem.as_ref().unwrap().map(),
            ServedShMem::Server(server) => server.shmem.as_ref().unwrap().map(),
        }
    }

    fn map_mut(&mut self) -> &mut [u8] {
        match self {
            ServedShMem::Client(client) => client.shmem.as_mut().unwrap().map_mut(),
            ServedShMem::Server(server) => server.shmem.as_mut().unwrap().map_mut(),
        }
    }
}

/// A request sent to the ShMem server to receive a fd to a shared map
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum AshmemRequest {
    /// Register a new map with a given size.
    NewMap(usize),
    /// Another client already has a map with this description mapped.
    ExistingMap(ShMemDescription),
    /// A client tells us it unregisters the previously allocated map
    Deregister(u32),
}

#[derive(Debug)]
pub struct AshmemClient {
    unix_socket_file: String,
}

#[derive(Debug)]
pub struct AshmemService {
    maps: HashMap<[u8; 20], UnixShMem>,
}

impl AshmemService {
    /// Create a new AshMem service
    #[must_use]
    fn new() -> Self {
        AshmemService {
            maps: HashMap::new(),
        }
    }

    /// Read and handle the client request, send the answer over unix fd.
    fn handle_client(&mut self, stream: &mut UnixStream) -> Result<(), Error> {
        // Always receive one be u32 of size, then the command.
        let mut size_bytes = [0u8; 4];
        stream.read_exact(&mut size_bytes)?;
        let size = u32::from_be_bytes(size_bytes);
        let mut bytes = vec![];
        bytes.resize(size as usize, 0u8);
        stream
            .read_exact(&mut bytes)
            .expect("Failed to read message body");
        let request: AshmemRequest = postcard::from_bytes(&bytes)?;

        // Handle the client request
        let (shmem_slice, fd): ([u8; 20], RawFd) = match request {
            AshmemRequest::NewMap(map_size) => match UnixShMem::new(map_size) {
                Err(e) => ([0; 20], -1),
                Ok(map) => {
                    let res = (*map.shm_slice(), map.shm_id);
                    self.maps.insert(*map.shm_slice(), map);
                    res
                }
            },
            AshmemRequest::ExistingMap(description) => {
                match self.maps.get(&description.str_bytes) {
                    None => ([0; 20], -1),
                    Some(map) => (*map.shm_slice(), map.shm_id),
                }
            }
            AshmemRequest::Deregister(_) => {
                return Ok(());
            }
        };

        stream.send_fds(&shmem_slice, &[fd])?;
        Ok(())
    }

    /// Create a new AshmemService, then listen and service incoming connections in a new thread.
    pub fn start() -> Result<thread::JoinHandle<()>, Error> {
        let syncpair = Arc::new((Mutex::new(false), Condvar::new()));
        let childsyncpair = Arc::clone(&syncpair);
        let res = thread::spawn(move || {
            Self::new()
                .listen(ASHMEM_SERVER_NAME, childsyncpair)
                .unwrap()
        });

        let (lock, cvar) = &*syncpair;
        let mut started = lock.lock().unwrap();
        while !*started {
            started = cvar.wait(started).unwrap();
        }
        Ok(res)
    }

    /// Listen on a filename (or abstract name) for new connections and serve them. This function
    /// should not return.
    fn listen(
        &mut self,
        filename: &str,
        syncpair: Arc<(Mutex<bool>, Condvar)>,
    ) -> Result<(), Error> {
        let listener = UnixListener::bind_unix_addr(&UnixSocketAddr::new(filename)?)?;
        let mut clients: HashMap<PollFd, (UnixStream, UnixSocketAddr)> = HashMap::new();
        let mut poll_fds: Vec<PollFd> = vec![PollFd::new(
            listener.as_raw_fd(),
            PollFlags::POLLIN | PollFlags::POLLRDNORM | PollFlags::POLLRDBAND,
        )];

        let (lock, cvar) = &*syncpair;
        *lock.lock().unwrap() = true;
        cvar.notify_one();

        loop {
            match poll(&mut poll_fds, -1) {
                Ok(num_fds) if num_fds > 0 => (),
                Ok(_) => continue,
                Err(e) => {
                    println!("Error polling for activity: {:?}", e);
                    continue;
                }
            };
            let copied_poll_fds: Vec<PollFd> = poll_fds.iter().copied().collect();
            for poll_fd in copied_poll_fds {
                let revents = poll_fd.revents().expect("revents should not be None");
                if revents.contains(PollFlags::POLLIN) {
                    if clients.contains_key(&poll_fd) {
                        let (stream, _addr) = clients.get_mut(&poll_fd).unwrap();
                        match self.handle_client(stream) {
                            Ok(()) => (),
                            Err(e) => {
                                dbg!("Ignoring failed read from client", e);
                                continue;
                            }
                        };
                    } else {
                        let (mut stream, addr) = match listener.accept_unix_addr() {
                            Ok(stream_val) => stream_val,
                            Err(e) => {
                                println!("Error accepting client: {:?}", e);
                                continue;
                            }
                        };

                        println!("Recieved connection from {:?}", addr);
                        let pollfd = PollFd::new(
                            stream.as_raw_fd(),
                            PollFlags::POLLIN | PollFlags::POLLRDNORM | PollFlags::POLLRDBAND,
                        );
                        poll_fds.push(pollfd);
                        match self.handle_client(&mut stream) {
                            Ok(()) => (),
                            Err(e) => {
                                dbg!("Ignoring failed read from client", e);
                            }
                        };
                        clients.insert(pollfd, (stream, addr));
                    }
                } else if revents.contains(PollFlags::POLLHUP) {
                    poll_fds.remove(poll_fds.iter().position(|item| *item == poll_fd).unwrap());
                    clients.remove(&poll_fd);
                } else {
                    println!("Unknwon revents flags: {:?}", revents);
                }
            }
        }
    }
}
