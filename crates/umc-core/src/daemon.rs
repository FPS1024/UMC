//! Local-only JSON-RPC transport for a persistent UMC Core process.
use crate::UmcCore;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use umc_api::{RpcRequest, RpcResponse};

const MAX_RPC_LINE: usize = 1024 * 1024;

async fn serve_connection<S>(core: UmcCore, stream: S) -> std::io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    loop {
        line.clear();
        let count = reader.read_line(&mut line).await?;
        if count == 0 {
            break;
        }
        if count > MAX_RPC_LINE {
            let response = error_response(None, -32600, "RPC message exceeds 1 MiB");
            write_response(&mut write, &response).await?;
            break;
        }
        let request = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(request) => request,
            Err(error) => {
                let response = error_response(None, -32700, &error.to_string());
                write_response(&mut write, &response).await?;
                continue;
            }
        };
        if request.id.is_none() {
            continue;
        }
        let response = core.dispatch(request).await;
        write_response(&mut write, &response).await?;
    }
    Ok(())
}

async fn write_response<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    response: &RpcResponse,
) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(response).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await
}

fn error_response(id: Option<serde_json::Value>, code: i32, message: &str) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(umc_api::RpcError {
            code,
            message: message.into(),
            data: None,
        }),
    }
}

#[cfg(unix)]
pub async fn serve_local(core: UmcCore) -> std::io::Result<()> {
    use std::os::unix::fs::FileTypeExt;
    use tokio::net::{UnixListener, UnixStream};

    let root = core.data_dir().to_path_buf();
    tokio::fs::create_dir_all(&root).await?;
    crate::engine::set_private_dir(&root).await.map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Could not secure UMC data directory: {e}"),
        )
    })?;
    let processes = root.join("processes");
    tokio::fs::create_dir_all(&processes).await?;
    crate::engine::set_private_dir(&processes)
        .await
        .map_err(std::io::Error::other)?;
    core.recover_processes()
        .await
        .map_err(|e| std::io::Error::other(format!("Could not recover process state: {e}")))?;
    let socket_path = root.join("umc.sock");
    if tokio::fs::symlink_metadata(&socket_path).await.is_ok() {
        let metadata = tokio::fs::symlink_metadata(&socket_path).await?;
        if !metadata.file_type().is_socket() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "UMC socket path exists and is not a socket",
            ));
        }
        match UnixStream::connect(&socket_path).await {
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "UMC daemon is already running",
                ));
            }
            Err(_) => tokio::fs::remove_file(&socket_path).await?,
        }
    }
    let listener = UnixListener::bind(&socket_path)
        .map_err(|e| std::io::Error::new(e.kind(), format!("Could not bind UMC socket: {e}")))?;
    crate::engine::set_private_file(&socket_path).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Could not secure UMC socket: {e}"),
        )
    })?;
    let _cleanup = SocketCleanup(socket_path.clone());
    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                break;
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let core = core.clone();
                tokio::spawn(async move { let _ = serve_connection(core, stream).await; });
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
struct SocketCleanup(std::path::PathBuf);

#[cfg(unix)]
impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(windows)]
pub async fn serve_local(core: UmcCore) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    const PIPE_NAME: &str = r"\\.\pipe\umc";
    let root = core.data_dir().to_path_buf();
    tokio::fs::create_dir_all(&root).await?;
    crate::engine::set_private_dir(&root)
        .await
        .map_err(std::io::Error::other)?;
    let processes = root.join("processes");
    tokio::fs::create_dir_all(&processes).await?;
    crate::engine::set_private_dir(&processes)
        .await
        .map_err(std::io::Error::other)?;
    core.recover_processes()
        .await
        .map_err(std::io::Error::other)?;
    let mut first = true;
    loop {
        let server = create_user_pipe(first)?;
        first = false;
        tokio::select! {
            signal = tokio::signal::ctrl_c() => { signal?; break; }
            connected = server.connect() => {
                connected?;
                let core = core.clone();
                tokio::spawn(async move { let _ = serve_connection(core, server).await; });
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn create_user_pipe(
    first: bool,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use std::{ffi::c_void, mem::size_of, ptr};
    use tokio::net::windows::named_pipe::ServerOptions;

    #[repr(C)]
    struct SecurityAttributes {
        length: u32,
        descriptor: *mut c_void,
        inherit_handle: i32,
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    // Owner Rights grants access to the user who created this named pipe, without
    // the default Everyone/anonymous read ACEs used by CreateNamedPipe.
    let sddl: Vec<u16> = "D:P(A;;GA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = ptr::null_mut();
    let mut descriptor_size = 0;
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            &mut descriptor_size,
        )
    };
    if converted == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut attributes = SecurityAttributes {
        length: size_of::<SecurityAttributes>() as u32,
        descriptor,
        inherit_handle: 0,
    };
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                r"\\.\pipe\umc",
                &mut attributes as *mut SecurityAttributes as *mut c_void,
            )
    };
    unsafe { LocalFree(descriptor) };
    let _ = descriptor_size;
    result
}

#[cfg(not(any(unix, windows)))]
pub async fn serve_local(_core: UmcCore) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Local daemon transport is unsupported on this platform",
    ))
}
