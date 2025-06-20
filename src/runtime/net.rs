// Runtime support for networking operations
// "Connecting Palladium to the world"

use std::net::{
    TcpStream as StdTcpStream, TcpListener as StdTcpListener,
    UdpSocket as StdUdpSocket, SocketAddr as StdSocketAddr,
    Ipv4Addr as StdIpv4Addr, Ipv6Addr as StdIpv6Addr,
    ToSocketAddrs, Shutdown as StdShutdown,
};
use std::io::{Read, Write};
use std::time::Duration;

/// Socket handle wrapper
#[repr(C)]
pub struct SocketHandle {
    socket_type: SocketType,
    handle: SocketData,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
enum SocketType {
    TcpStream = 0,
    TcpListener = 1,
    UdpSocket = 2,
}

enum SocketData {
    TcpStream(StdTcpStream),
    TcpListener(StdTcpListener),
    UdpSocket(StdUdpSocket),
}

/// Socket address representation
#[repr(C)]
pub struct SocketAddr {
    family: u8,  // 4 for IPv4, 6 for IPv6
    port: u16,
    addr: [u8; 16], // IPv4 uses first 4 bytes, IPv6 uses all 16
    flowinfo: u32,  // IPv6 only
    scope_id: u32,  // IPv6 only
}

impl SocketAddr {
    fn to_std(&self) -> Option<StdSocketAddr> {
        match self.family {
            4 => {
                let addr = StdIpv4Addr::new(self.addr[0], self.addr[1], self.addr[2], self.addr[3]);
                Some(StdSocketAddr::from((addr, self.port)))
            }
            6 => {
                let addr = StdIpv6Addr::new(
                    u16::from_be_bytes([self.addr[0], self.addr[1]]),
                    u16::from_be_bytes([self.addr[2], self.addr[3]]),
                    u16::from_be_bytes([self.addr[4], self.addr[5]]),
                    u16::from_be_bytes([self.addr[6], self.addr[7]]),
                    u16::from_be_bytes([self.addr[8], self.addr[9]]),
                    u16::from_be_bytes([self.addr[10], self.addr[11]]),
                    u16::from_be_bytes([self.addr[12], self.addr[13]]),
                    u16::from_be_bytes([self.addr[14], self.addr[15]]),
                );
                Some(StdSocketAddr::from((addr, self.port)))
            }
            _ => None,
        }
    }

    fn from_std(addr: &StdSocketAddr) -> SocketAddr {
        match addr {
            StdSocketAddr::V4(v4) => {
                let octets = v4.ip().octets();
                let mut addr_bytes = [0u8; 16];
                addr_bytes[0..4].copy_from_slice(&octets);
                
                SocketAddr {
                    family: 4,
                    port: v4.port(),
                    addr: addr_bytes,
                    flowinfo: 0,
                    scope_id: 0,
                }
            }
            StdSocketAddr::V6(v6) => {
                let segments = v6.ip().segments();
                let mut addr_bytes = [0u8; 16];
                for (i, &seg) in segments.iter().enumerate() {
                    let bytes = seg.to_be_bytes();
                    addr_bytes[i * 2] = bytes[0];
                    addr_bytes[i * 2 + 1] = bytes[1];
                }
                
                SocketAddr {
                    family: 6,
                    port: v6.port(),
                    addr: addr_bytes,
                    flowinfo: v6.flowinfo(),
                    scope_id: v6.scope_id(),
                }
            }
        }
    }
}

// TCP Stream operations

#[no_mangle]
pub extern "C" fn pd_tcp_connect(addr: *const SocketAddr) -> *mut SocketHandle {
    if addr.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let addr = &*addr;
        if let Some(std_addr) = addr.to_std() {
            match StdTcpStream::connect(std_addr) {
                Ok(stream) => {
                    let handle = Box::new(SocketHandle {
                        socket_type: SocketType::TcpStream,
                        handle: SocketData::TcpStream(stream),
                    });
                    Box::into_raw(handle)
                }
                Err(_) => std::ptr::null_mut(),
            }
        } else {
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_connect_timeout(addr: *const SocketAddr, timeout_ms: u64) -> *mut SocketHandle {
    if addr.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let addr = &*addr;
        if let Some(std_addr) = addr.to_std() {
            let timeout = Duration::from_millis(timeout_ms);
            match StdTcpStream::connect_timeout(&std_addr, timeout) {
                Ok(stream) => {
                    let handle = Box::new(SocketHandle {
                        socket_type: SocketType::TcpStream,
                        handle: SocketData::TcpStream(stream),
                    });
                    Box::into_raw(handle)
                }
                Err(_) => std::ptr::null_mut(),
            }
        } else {
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_read(handle: *mut SocketHandle, buffer: *mut u8, len: usize) -> i64 {
    if handle.is_null() || buffer.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        match &mut handle.handle {
            SocketData::TcpStream(stream) => {
                let buffer_slice = std::slice::from_raw_parts_mut(buffer, len);
                match stream.read(buffer_slice) {
                    Ok(n) => n as i64,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_write(handle: *mut SocketHandle, buffer: *const u8, len: usize) -> i64 {
    if handle.is_null() || buffer.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        match &mut handle.handle {
            SocketData::TcpStream(stream) => {
                let buffer_slice = std::slice::from_raw_parts(buffer, len);
                match stream.write(buffer_slice) {
                    Ok(n) => n as i64,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_shutdown(handle: *mut SocketHandle, how: u8) -> i32 {
    if handle.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        match &handle.handle {
            SocketData::TcpStream(stream) => {
                let shutdown = match how {
                    0 => StdShutdown::Read,
                    1 => StdShutdown::Write,
                    2 => StdShutdown::Both,
                    _ => return -1,
                };
                match stream.shutdown(shutdown) {
                    Ok(_) => 0,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_peer_addr(handle: *mut SocketHandle, addr: *mut SocketAddr) -> i32 {
    if handle.is_null() || addr.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::TcpStream(stream) => {
                match stream.peer_addr() {
                    Ok(peer) => {
                        *addr = SocketAddr::from_std(&peer);
                        0
                    }
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_local_addr(handle: *mut SocketHandle, addr: *mut SocketAddr) -> i32 {
    if handle.is_null() || addr.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::TcpStream(stream) => {
                match stream.local_addr() {
                    Ok(local) => {
                        *addr = SocketAddr::from_std(&local);
                        0
                    }
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_set_nodelay(handle: *mut SocketHandle, nodelay: u8) -> i32 {
    if handle.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::TcpStream(stream) => {
                match stream.set_nodelay(nodelay != 0) {
                    Ok(_) => 0,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_set_ttl(handle: *mut SocketHandle, ttl: u32) -> i32 {
    if handle.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::TcpStream(stream) => {
                match stream.set_ttl(ttl) {
                    Ok(_) => 0,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

// TCP Listener operations

#[no_mangle]
pub extern "C" fn pd_tcp_bind(addr: *const SocketAddr) -> *mut SocketHandle {
    if addr.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let addr = &*addr;
        if let Some(std_addr) = addr.to_std() {
            match StdTcpListener::bind(std_addr) {
                Ok(listener) => {
                    let handle = Box::new(SocketHandle {
                        socket_type: SocketType::TcpListener,
                        handle: SocketData::TcpListener(listener),
                    });
                    Box::into_raw(handle)
                }
                Err(_) => std::ptr::null_mut(),
            }
        } else {
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_tcp_accept(handle: *mut SocketHandle, client_addr: *mut SocketAddr) -> *mut SocketHandle {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::TcpListener(listener) => {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        if !client_addr.is_null() {
                            *client_addr = SocketAddr::from_std(&addr);
                        }
                        
                        let client_handle = Box::new(SocketHandle {
                            socket_type: SocketType::TcpStream,
                            handle: SocketData::TcpStream(stream),
                        });
                        Box::into_raw(client_handle)
                    }
                    Err(_) => std::ptr::null_mut(),
                }
            }
            _ => std::ptr::null_mut(),
        }
    }
}

// UDP Socket operations

#[no_mangle]
pub extern "C" fn pd_udp_bind(addr: *const SocketAddr) -> *mut SocketHandle {
    if addr.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let addr = &*addr;
        if let Some(std_addr) = addr.to_std() {
            match StdUdpSocket::bind(std_addr) {
                Ok(socket) => {
                    let handle = Box::new(SocketHandle {
                        socket_type: SocketType::UdpSocket,
                        handle: SocketData::UdpSocket(socket),
                    });
                    Box::into_raw(handle)
                }
                Err(_) => std::ptr::null_mut(),
            }
        } else {
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_udp_send_to(handle: *mut SocketHandle, buffer: *const u8, len: usize, addr: *const SocketAddr) -> i64 {
    if handle.is_null() || buffer.is_null() || addr.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        let addr = &*addr;
        
        match &handle.handle {
            SocketData::UdpSocket(socket) => {
                if let Some(std_addr) = addr.to_std() {
                    let buffer_slice = std::slice::from_raw_parts(buffer, len);
                    match socket.send_to(buffer_slice, std_addr) {
                        Ok(n) => n as i64,
                        Err(_) => -1,
                    }
                } else {
                    -1
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_udp_recv_from(handle: *mut SocketHandle, buffer: *mut u8, len: usize, addr: *mut SocketAddr) -> i64 {
    if handle.is_null() || buffer.is_null() || addr.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::UdpSocket(socket) => {
                let buffer_slice = std::slice::from_raw_parts_mut(buffer, len);
                match socket.recv_from(buffer_slice) {
                    Ok((n, src_addr)) => {
                        *addr = SocketAddr::from_std(&src_addr);
                        n as i64
                    }
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_udp_connect(handle: *mut SocketHandle, addr: *const SocketAddr) -> i32 {
    if handle.is_null() || addr.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        let addr = &*addr;
        
        match &handle.handle {
            SocketData::UdpSocket(socket) => {
                if let Some(std_addr) = addr.to_std() {
                    match socket.connect(std_addr) {
                        Ok(_) => 0,
                        Err(_) => -1,
                    }
                } else {
                    -1
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_udp_send(handle: *mut SocketHandle, buffer: *const u8, len: usize) -> i64 {
    if handle.is_null() || buffer.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::UdpSocket(socket) => {
                let buffer_slice = std::slice::from_raw_parts(buffer, len);
                match socket.send(buffer_slice) {
                    Ok(n) => n as i64,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_udp_recv(handle: *mut SocketHandle, buffer: *mut u8, len: usize) -> i64 {
    if handle.is_null() || buffer.is_null() {
        return -1;
    }

    unsafe {
        let handle = &*handle;
        match &handle.handle {
            SocketData::UdpSocket(socket) => {
                let buffer_slice = std::slice::from_raw_parts_mut(buffer, len);
                match socket.recv(buffer_slice) {
                    Ok(n) => n as i64,
                    Err(_) => -1,
                }
            }
            _ => -1,
        }
    }
}

// Common socket operations

#[no_mangle]
pub extern "C" fn pd_socket_close(handle: *mut SocketHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }
    
    unsafe {
        let _ = Box::from_raw(handle);
        0
    }
}

// DNS and address resolution

#[no_mangle]
pub extern "C" fn pd_lookup_host(host: *const u8, host_len: usize, addrs: *mut *mut SocketAddr, count: *mut usize) -> i32 {
    if host.is_null() || addrs.is_null() || count.is_null() {
        return -1;
    }

    unsafe {
        let host_slice = std::slice::from_raw_parts(host, host_len);
        let host_str = match std::str::from_utf8(host_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match (host_str, 0).to_socket_addrs() {
            Ok(iter) => {
                let addr_vec: Vec<SocketAddr> = iter
                    .map(|addr| SocketAddr::from_std(&addr))
                    .collect();
                
                *count = addr_vec.len();
                let addrs_array = addr_vec.into_boxed_slice();
                *addrs = Box::into_raw(addrs_array) as *mut SocketAddr;
                
                0
            }
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_parse_socket_addr(addr_str: *const u8, addr_len: usize, addr: *mut SocketAddr) -> i32 {
    if addr_str.is_null() || addr.is_null() {
        return -1;
    }

    unsafe {
        let addr_slice = std::slice::from_raw_parts(addr_str, addr_len);
        let addr_string = match std::str::from_utf8(addr_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        // Try to parse as socket address
        if let Ok(parsed) = addr_string.parse::<StdSocketAddr>() {
            *addr = SocketAddr::from_std(&parsed);
            0
        } else {
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_free_socket_addrs(addrs: *mut SocketAddr, count: usize) {
    if addrs.is_null() {
        return;
    }
    
    unsafe {
        let _ = Box::from_raw(std::slice::from_raw_parts_mut(addrs, count));
    }
}