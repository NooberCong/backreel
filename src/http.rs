//! HTTPS GET through WinHTTP, so TLS and proxy handling come from Windows.

use crate::Result;
use std::ffi::c_void;
use windows::Win32::Networking::NetworkListManager::{
    INetworkCostManager, NLM_CONNECTION_COST_APPROACHINGDATALIMIT, NLM_CONNECTION_COST_FIXED,
    NLM_CONNECTION_COST_OVERDATALIMIT, NLM_CONNECTION_COST_ROAMING, NLM_CONNECTION_COST_VARIABLE, NetworkListManager,
};
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{HSTRING, PCWSTR, w};

pub const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

const USER_AGENT: &str = concat!("backreel/", env!("CARGO_PKG_VERSION"));
const CONNECT_TIMEOUT_MS: i32 = 10_000;
const TRANSFER_TIMEOUT_MS: i32 = 30_000;

/// Closes a WinHTTP handle on drop.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void) -> Result<Self> {
        if raw.is_null() { Err(windows::core::Error::from_thread().into()) } else { Ok(Self(raw)) }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from a successful WinHTTP open call and is closed once.
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// Downloads `url`, failing on any non-200 status or a body over `max_bytes`.
pub fn get(url: &str, max_bytes: usize) -> Result<Vec<u8>> {
    let (host_name, path) = split_url(url)?;
    let agent = HSTRING::from(USER_AGENT);
    let host = HSTRING::from(host_name);
    let path = HSTRING::from(path);

    // SAFETY: every pointer passed is valid for the duration of its call, and
    // each handle outlives the handles opened from it (drop order is reversed).
    unsafe {
        let session =
            Handle::new(WinHttpOpen(&agent, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0))?;
        WinHttpSetTimeouts(
            session.0,
            CONNECT_TIMEOUT_MS,
            CONNECT_TIMEOUT_MS,
            TRANSFER_TIMEOUT_MS,
            TRANSFER_TIMEOUT_MS,
        )?;
        let protocols = WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2 | WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3;
        WinHttpSetOption(Some(session.0), WINHTTP_OPTION_SECURE_PROTOCOLS, Some(&protocols.to_ne_bytes()))?;

        let connection = Handle::new(WinHttpConnect(session.0, &host, INTERNET_DEFAULT_HTTPS_PORT, 0))?;
        let request = Handle::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            &path,
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0)?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())?;

        let status = status_code(&request)?;
        if status != 200 {
            return Err(format!("HTTP {status} from {host_name}").into());
        }
        read_body(&request, max_bytes)
    }
}

unsafe fn status_code(request: &Handle) -> Result<u32> {
    let mut status = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `status` and `size` are valid for writes and sized as the flags require.
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&raw mut status as *mut c_void),
            &mut size,
            std::ptr::null_mut(),
        )?;
    }
    Ok(status)
}

unsafe fn read_body(request: &Handle, max_bytes: usize) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let mut read = 0u32;
        // SAFETY: `chunk` is valid for writes of `chunk.len()` bytes.
        unsafe { WinHttpReadData(request.0, chunk.as_mut_ptr().cast(), chunk.len() as u32, &mut read)? };
        if read == 0 {
            return Ok(body);
        }
        if body.len() + read as usize > max_bytes {
            return Err(format!("response exceeds {max_bytes} bytes").into());
        }
        body.extend_from_slice(&chunk[..read as usize]);
    }
}

/// Splits an https URL into host and path-with-query. Plain http is refused.
fn split_url(url: &str) -> Result<(&str, &str)> {
    let rest = url.strip_prefix("https://").ok_or_else(|| format!("not an https URL: {url}"))?;
    let (host, path) = match rest.find('/') {
        Some(slash) => rest.split_at(slash),
        None => (rest, "/"),
    };
    if host.is_empty() {
        return Err(format!("URL has no host: {url}").into());
    }
    Ok((host, path))
}

/// True when Windows reports a connection the user pays for by usage.
/// An unavailable answer counts as unmetered so a daily run is not lost.
pub fn is_metered() -> bool {
    let cost = || -> windows::core::Result<u32> {
        // SAFETY: COM is initialised by the caller; a null destination asks for the machine-wide cost.
        unsafe {
            let manager: INetworkCostManager = CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL)?;
            let mut cost = 0;
            manager.GetCost(&mut cost, std::ptr::null())?;
            Ok(cost)
        }
    };
    let metered = NLM_CONNECTION_COST_FIXED.0
        | NLM_CONNECTION_COST_VARIABLE.0
        | NLM_CONNECTION_COST_ROAMING.0
        | NLM_CONNECTION_COST_APPROACHINGDATALIMIT.0
        | NLM_CONNECTION_COST_OVERDATALIMIT.0;
    cost().is_ok_and(|c| c & metered as u32 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_host_from_path_and_query() {
        assert_eq!(split_url("https://a.example/b?c=d").unwrap(), ("a.example", "/b?c=d"));
        assert_eq!(split_url("https://a.example").unwrap(), ("a.example", "/"));
    }

    // These reach the real network: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_download_succeeds() {
        assert!(!get("https://www.bing.com/robots.txt", MAX_JSON_BYTES).unwrap().is_empty());
    }

    #[test]
    #[ignore]
    fn live_http_errors_are_reported() {
        let error = get("https://wallhaven.cc/api/v1/does-not-exist", MAX_JSON_BYTES).unwrap_err();
        assert!(error.to_string().contains("HTTP 404"), "{error}");
    }

    #[test]
    #[ignore]
    fn live_unknown_host_is_an_error() {
        assert!(get("https://backreel-no-such-host.invalid/", MAX_JSON_BYTES).is_err());
    }

    #[test]
    #[ignore]
    fn live_size_cap_stops_the_download() {
        let error = get("https://www.bing.com/robots.txt", 10).unwrap_err();
        assert!(error.to_string().contains("exceeds"), "{error}");
    }

    #[test]
    fn refuses_anything_but_https() {
        assert!(split_url("http://a.example/").is_err());
        assert!(split_url("https:///path").is_err());
    }
}
