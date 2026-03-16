//! Reverse DNS lookup for IP direct-connect blocking (F5).

/// Attempt reverse DNS lookup for an IP address using system `getnameinfo`.
/// Returns the resolved hostname, or `None` if lookup fails or returns the
/// raw IP string (no PTR record).
#[cfg(unix)]
pub(super) fn reverse_dns_lookup(ip: &std::net::IpAddr) -> Option<String> {
    use std::net::IpAddr;

    unsafe {
        let mut host_buf = [0u8; 1025]; // NI_MAXHOST

        let ret = match ip {
            IpAddr::V4(ipv4) => {
                let mut sa: libc::sockaddr_in = std::mem::zeroed();
                sa.sin_family = libc::AF_INET as libc::sa_family_t;
                sa.sin_addr.s_addr = u32::from_ne_bytes(ipv4.octets());
                libc::getnameinfo(
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                    host_buf.as_mut_ptr() as *mut libc::c_char,
                    host_buf.len() as libc::socklen_t,
                    std::ptr::null_mut(),
                    0,
                    libc::NI_NAMEREQD,
                )
            }
            IpAddr::V6(ipv6) => {
                let mut sa: libc::sockaddr_in6 = std::mem::zeroed();
                sa.sin6_family = libc::AF_INET6 as libc::sa_family_t;
                sa.sin6_addr.s6_addr = ipv6.octets();
                libc::getnameinfo(
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
                    host_buf.as_mut_ptr() as *mut libc::c_char,
                    host_buf.len() as libc::socklen_t,
                    std::ptr::null_mut(),
                    0,
                    libc::NI_NAMEREQD,
                )
            }
        };

        if ret != 0 {
            return None;
        }

        let c_str = std::ffi::CStr::from_ptr(host_buf.as_ptr() as *const libc::c_char);
        c_str.to_str().ok().map(|s| s.to_string())
    }
}

#[cfg(not(unix))]
pub(super) fn reverse_dns_lookup(_ip: &std::net::IpAddr) -> Option<String> {
    None
}
